use crate::cdp::{self, ChromeProcess, LaunchOptions};
use crate::cdp_client::CdpClient;
use crate::cdp_page::PageSession;
use crate::extractor::{self, ExtractorConfig};
use crate::logging::Logger;
use crate::models::{ExtractPreview, RawWork};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DOUYIN_HOME: &str = "https://www.douyin.com/";
pub const CREATOR_HOME: &str = DOUYIN_HOME;

/// 一个浏览器 + 页面会话，用于采集结束后立即关闭，避免长期占用账号数据目录。
pub struct Browser {
    chrome: Option<ChromeProcess>,
    pub client: CdpClient,
    pub page: PageSession,
}

impl Browser {
    pub async fn open(
        exe: PathBuf,
        profile_dir: PathBuf,
        headless: bool,
        logger: &Logger,
    ) -> Result<Self> {
        // 若该账号的浏览器数据目录已有开启调试端口的实例（例如用户手动打开的登录窗口），
        // 直接复用它，避免同一目录二次启动失败。
        let probe = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();
        if let Some(ws_url) = cdp::live_debugger_url(&profile_dir, &probe).await {
            match CdpClient::connect(&ws_url).await {
                Ok(client) => match PageSession::open_or_reuse(&client, "douyin.com", logger, headless).await {
                    Ok(page) => {
                        logger.info("browser", "已复用正在运行的浏览器实例");
                        return Ok(Self {
                            chrome: None,
                            client,
                            page,
                        });
                    }
                    Err(err) => logger.warn("browser", format!("复用浏览器实例失败，改为新开实例：{err:#}")),
                },
                Err(err) => logger.warn("browser", format!("连接已有调试端口失败：{err:#}")),
            }
        }

        let probe_dir = profile_dir.clone();
        let options = LaunchOptions {
            exe,
            user_data_dir: profile_dir,
            headless,
            initial_url: None,
            extra_args: vec!["about:blank".to_string()],
        };
        let mut chrome = tokio::task::spawn_blocking(move || cdp::launch(&options))
            .await
            .map_err(|err| anyhow!("浏览器启动任务失败：{err}"))??;
        logger.info(
            "browser",
            format!("浏览器已启动（端口 {}，无界面={headless}）", chrome.port),
        );

        // 启动日志里的端口可能已经失效：Chrome 会「换壳」重启并重写 DevToolsActivePort。
        // 这里以文件中的最新端口为准，并确认 HTTP 端点真正可用后再连接。
        let resolved = cdp::resolve_debugger(&probe_dir, &probe, Duration::from_secs(20)).await;
        let (port, ws_url) = match resolved {
            Some(resolved) => resolved,
            None => {
                chrome.kill();
                return Err(anyhow!(
                    "浏览器调试端口在 20 秒内未就绪，请重试；若反复出现请检查浏览器版本"
                ));
            }
        };
        if port != chrome.port {
            logger.info("browser", format!("调试端口已更新为 {port}"));
        }
        chrome.port = port;
        chrome.ws_url = ws_url.clone();

        let client = CdpClient::connect(&ws_url).await?;
        let page = PageSession::open(&client, logger, headless).await?;
        Ok(Self {
            chrome: Some(chrome),
            client,
            page,
        })
    }

    pub async fn goto(&self, url: &str, wait: Duration, ready_timeout: Duration) -> Result<()> {
        self.page.navigate(url).await?;
        self.page.wait_ready(ready_timeout).await?;
        tokio::time::sleep(wait).await;
        Ok(())
    }

    /// 导航至目标页面；若提供了早退条件（如已拦截到网络数据），则一旦命中立即返回，大幅缩减采集等待时间。
    pub async fn goto_or_until<F>(&self, url: &str, max_wait: Duration, until: F) -> Result<()>
    where
        F: Fn() -> bool,
    {
        self.page.navigate(url).await?;
        let deadline = Instant::now() + max_wait;
        while Instant::now() < deadline {
            if until() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let _ = self.page.wait_ready(Duration::from_secs(5)).await;
        Ok(())
    }

    pub async fn shutdown(mut self) {
        self.page.close().await;
        if let Some(chrome) = self.chrome.as_mut() {
            chrome.kill();
        }
    }
}

pub async fn extract(
    page: &PageSession,
    config: &ExtractorConfig,
    custom_script: &str,
) -> Result<extractor::ParsedPage> {
    let script = config.script(custom_script);
    let value = page.evaluate(&script).await?;
    extractor::parse_page(&value)
}

/// 试采集：只解析不写库，用于验证页面结构与登录状态。
pub async fn preview(
    exe: PathBuf,
    profile_dir: PathBuf,
    headless: bool,
    settings: &crate::settings::Settings,
    account_url: &str,
    custom_script: &str,
    logger: &Logger,
) -> Result<ExtractPreview> {
    let browser = Browser::open(exe, profile_dir, headless, logger).await?;
    let url = if account_url.trim().is_empty() {
        DOUYIN_HOME.to_string()
    } else {
        account_url.trim().to_string()
    };
    let result = async {
        browser
            .goto(
                &url,
                Duration::from_secs(settings.page_wait_secs.max(2) as u64),
                Duration::from_secs(45),
            )
            .await?;
        if settings.scroll_rounds > 0 {
            browser
                .page
                .scroll_to_bottom(settings.scroll_rounds as u32, Duration::from_millis(1200))
                .await?;
        }
        let config = ExtractorConfig::from_settings(settings);
        extract(&browser.page, &config, custom_script)
            .await
            .map(|parsed| parsed.preview)
    }
    .await;
    browser.shutdown().await;
    result
}

/// 解析网络接口返回的响应体 JSON 为 RawWork。
pub fn parse_cdp_body_to_raw_work(body_val: &serde_json::Value, expected_id: &str) -> Result<RawWork> {
    let body_str = body_val
        .get("body")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("网络响应体缺少 body 字段"))?;

    let is_base64 = body_val
        .get("base64Encoded")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let json_text = if is_base64 {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(body_str)?;
        String::from_utf8(bytes)?
    } else {
        body_str.to_string()
    };

    let parsed_json: serde_json::Value = serde_json::from_str(&json_text)?;
    crate::extractor::parse_aweme_detail_json(&parsed_json, expected_id)
}

/// 尝试通过 CDP 网络层读取 /aweme/v1/web/aweme/detail/ 请求的响应体并解析为 RawWork。
async fn try_get_detail_from_cdp(
    browser: &Browser,
    request_id: &str,
    expected_id: &str,
) -> Result<RawWork> {
    let body_val = browser
        .page
        .send(
            "Network.getResponseBody",
            serde_json::json!({ "requestId": request_id }),
        )
        .await?;
    parse_cdp_body_to_raw_work(&body_val, expected_id)
}

/// 采集账号作品，返回解析结果、整体预览与单视频错误明细（aweme_id -> error_reason）。
pub async fn fetch_works(
    exe: PathBuf,
    profile_dir: PathBuf,
    settings: &crate::settings::Settings,
    tracked: &[String],
    logger: &Logger,
) -> Result<(Vec<RawWork>, ExtractPreview, HashMap<String, String>)> {
    if tracked.is_empty() {
        return Err(anyhow!("请先配置监控视频"));
    }
    let browser = Browser::open(exe, profile_dir, settings.headless, logger).await?;
    let result = async {
        let mut works = Vec::new();
        let mut preview = ExtractPreview::default();
        let mut item_errors = HashMap::new();

        for id in tracked {
            let url = format!("https://www.douyin.com/video/{id}");
            logger.info("collect", format!("正在读取目标视频页面: {url}"));

            // 1. 优先启动 CDP 网络监听，捕获 /aweme/v1/web/aweme/detail/
            let captured_work: std::sync::Arc<tokio::sync::Mutex<Option<RawWork>>> =
                std::sync::Arc::new(tokio::sync::Mutex::new(None));
            let captured_work_clone = captured_work.clone();
            let captured_net_req_id: std::sync::Arc<tokio::sync::Mutex<Option<String>>> =
                std::sync::Arc::new(tokio::sync::Mutex::new(None));
            let captured_net_clone = captured_net_req_id.clone();

            let mut rx = browser.client.subscribe();
            let target_session = browser.page.session_id.clone();
            let client_clone = browser.client.clone();
            let expected_id = id.clone();
            let logger_clone = logger.clone();
            let _ = browser.page.enable_fetch_interception().await;

            let network_listener = tokio::spawn(async move {
                let deadline = std::time::Instant::now() + Duration::from_secs(50);
                while std::time::Instant::now() < deadline {
                    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                    match tokio::time::timeout(remaining, rx.recv()).await {
                        Ok(Ok(event)) => {
                            if event.session_id.as_deref() != Some(&target_session) {
                                continue;
                            }

                            // 1. Fetch 域精准拦截（Chromium 在 Response 阶段暂停，内存响应体绝对保留，彻底杜绝 No resource found）
                            if event.method == "Fetch.requestPaused" {
                                let req_id = event.params.get("requestId").and_then(|v| v.as_str());
                                let req_url = event
                                    .params
                                    .get("request")
                                    .and_then(|r| r.get("url"))
                                    .and_then(|u| u.as_str())
                                    .unwrap_or_default();
                                let status_code = event
                                    .params
                                    .get("responseStatusCode")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);

                                if let Some(req_id) = req_id {
                                    if req_url.contains("/aweme/v1/web/aweme/detail")
                                        && (status_code == 200 || status_code == 0)
                                    {
                                        match client_clone
                                            .send(
                                                "Fetch.getResponseBody",
                                                serde_json::json!({ "requestId": req_id }),
                                                Some(&target_session),
                                            )
                                            .await
                                        {
                                            Ok(body_val) => {
                                                match parse_cdp_body_to_raw_work(&body_val, &expected_id) {
                                                    Ok(work) => {
                                                        logger_clone.info(
                                                            "collect",
                                                            format!(
                                                                "网络拦截获取视频接口成功 ({expected_id}): 作者={} 点赞={:?} 评论={:?} 转发={:?} 收藏={:?}",
                                                                work.author_name,
                                                                work.metrics.like,
                                                                work.metrics.comment,
                                                                work.metrics.share,
                                                                work.metrics.collect
                                                            ),
                                                        );
                                                        let mut lock = captured_work_clone.lock().await;
                                                        *lock = Some(work);
                                                    }
                                                    Err(err) => {
                                                        logger_clone.warn(
                                                            "collect",
                                                            format!("网络拦截响应体解析异常 ({expected_id}): {err}"),
                                                        );
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                logger_clone.warn(
                                                    "collect",
                                                    format!("网络拦截读取响应体失败 ({expected_id}): {err}"),
                                                );
                                            }
                                        }
                                    }

                                    // 放行暂停的请求以保证页面正常加载
                                    if client_clone
                                        .send(
                                            "Fetch.continueResponse",
                                            serde_json::json!({ "requestId": req_id }),
                                            Some(&target_session),
                                        )
                                        .await
                                        .is_err()
                                    {
                                        let _ = client_clone
                                            .send(
                                                "Fetch.continueRequest",
                                                serde_json::json!({ "requestId": req_id }),
                                                Some(&target_session),
                                            )
                                            .await;
                                    }

                                    let lock = captured_work_clone.lock().await;
                                    if lock.is_some() {
                                        break;
                                    }
                                }
                            }

                            // 2. Network 域实时捕获，在 loadingFinished 到达时立刻提取 body（防止被后续视频切片冲刷淘汰）
                            if event.method == "Network.responseReceived" {
                                if let Some(req_url) = event
                                    .params
                                    .get("response")
                                    .and_then(|r| r.get("url"))
                                    .and_then(|u| u.as_str())
                                {
                                    if req_url.contains("/aweme/v1/web/aweme/detail") {
                                        if let Some(req_id) =
                                            event.params.get("requestId").and_then(|v| v.as_str())
                                        {
                                            let mut lock = captured_net_clone.lock().await;
                                            *lock = Some(req_id.to_string());
                                        }
                                    }
                                }
                            } else if event.method == "Network.loadingFinished" {
                                if let Some(req_id) =
                                    event.params.get("requestId").and_then(|v| v.as_str())
                                {
                                    let is_target = {
                                        let lock = captured_net_clone.lock().await;
                                        lock.as_deref() == Some(req_id)
                                    };
                                    if is_target {
                                        let body_res = client_clone
                                            .send(
                                                "Network.getResponseBody",
                                                serde_json::json!({ "requestId": req_id }),
                                                Some(&target_session),
                                            )
                                            .await;
                                        if let Ok(body_val) = body_res {
                                            if let Ok(work) =
                                                parse_cdp_body_to_raw_work(&body_val, &expected_id)
                                            {
                                                logger_clone.info(
                                                    "collect",
                                                    format!(
                                                        "网络监听获取视频接口成功 ({expected_id}): 作者={} 点赞={:?} 评论={:?} 转发={:?} 收藏={:?}",
                                                        work.author_name,
                                                        work.metrics.like,
                                                        work.metrics.comment,
                                                        work.metrics.share,
                                                        work.metrics.collect
                                                    ),
                                                );
                                                let mut lock = captured_work_clone.lock().await;
                                                *lock = Some(work);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                            // 消息堆积时忽略丢帧，继续监听
                            continue;
                        }
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                        Err(_) => break,
                    }
                }
                let _ = client_clone.send("Fetch.disable", serde_json::json!({}), Some(&target_session)).await;
            });

            let is_captured = {
                let captured = captured_work.clone();
                move || captured.try_lock().map(|l| l.is_some()).unwrap_or(false)
            };

            if let Err(err) = browser
                .goto_or_until(
                    &url,
                    Duration::from_secs(settings.page_wait_secs.max(2) as u64),
                    is_captured,
                )
                .await
            {
                logger.warn("collect", format!("打开视频页面失败 ({id}): {err}"));
                item_errors.insert(id.clone(), format!("页面访问超时或失败：{err}"));
                network_listener.abort();
                let _ = browser.page.disable_fetch_interception().await;
                continue;
            }

            // 2. 检查网络拦截是否已拿到接口数据
            let mut final_work = {
                let lock = captured_work.lock().await;
                lock.clone()
            };

            // 如果尚未拿到，等待最多 2 秒以防事件在排队
            if final_work.is_none() {
                let _ = tokio::time::timeout(Duration::from_secs(2), network_listener).await;
                let lock = captured_work.lock().await;
                final_work = lock.clone();
            }

            // 如果仍未拿到但有 Network requestId，尝试兜底获取
            if final_work.is_none() {
                let req_id_opt = {
                    let lock = captured_net_req_id.lock().await;
                    lock.clone()
                };
                if let Some(req_id) = req_id_opt {
                    for attempt in 0..3 {
                        match try_get_detail_from_cdp(&browser, &req_id, id).await {
                            Ok(work) => {
                                logger.info(
                                    "collect",
                                    format!(
                                        "接口网络响应体兜底读取成功 ({id}): 点赞={:?} 评论={:?} 转发={:?} 收藏={:?}",
                                        work.metrics.like,
                                        work.metrics.comment,
                                        work.metrics.share,
                                        work.metrics.collect
                                    ),
                                );
                                final_work = Some(work);
                                break;
                            }
                            Err(err) => {
                                let err_msg = err.to_string();
                                if err_msg.contains("下架") || err_msg.contains("私密") {
                                    item_errors.insert(id.clone(), err_msg);
                                    break;
                                }
                                if attempt < 2 {
                                    tokio::time::sleep(Duration::from_millis(300)).await;
                                } else {
                                    logger.warn(
                                        "collect",
                                        format!("网络响应解析未成功 ({id}): {err}，回退到页面脚本解析"),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            if let Some(work) = final_work {
                works.push(work);
                let _ = browser.page.disable_fetch_interception().await;
                continue;
            }

            let _ = browser.page.disable_fetch_interception().await;

            // 3. 兜底：网络监听未命中时，使用注入脚本解析页面 DOM / 状态
            let script = crate::extractor_js::SINGLE_VIDEO_SCRIPT.replace("__ID__", &serde_json::to_string(id)?);
            let value = match browser.page.evaluate(&script).await {
                Ok(v) => v,
                Err(err) => {
                    logger.warn("collect", format!("执行解析脚本失败 ({id}): {err}"));
                    item_errors.insert(id.clone(), format!("页面脚本执行失败：{err}"));
                    continue;
                }
            };

            let parsed = match extractor::parse_page(&value) {
                Ok(p) => p,
                Err(err) => {
                    logger.warn("collect", format!("解析作品数据异常 ({id}): {err}"));
                    item_errors.insert(id.clone(), format!("数据格式解析异常：{err}"));
                    continue;
                }
            };

            preview = parsed.preview;
            if preview.blocked {
                item_errors.insert(id.clone(), "页面要求安全验证（需人工处理滑块）".to_string());
                logger.warn("collect", format!("遇到安全验证拦截 ({id})，中止后续视频采集"));
                break;
            }

            let matched: Vec<RawWork> = parsed.works.into_iter().filter(|w| w.aweme_id == *id).collect();
            if matched.is_empty() {
                let reason = if !preview.reason.is_empty() {
                    preview.reason.clone()
                } else {
                    "该视频页面未解析到有效数据".to_string()
                };
                item_errors.insert(id.clone(), reason);
            } else {
                works.extend(matched);
            }
        }

        preview.works = works.len();
        preview.ok = !works.is_empty() && !preview.blocked;
        preview.sample = works.iter().take(20).cloned().collect();
        Ok::<_, anyhow::Error>((works, preview, item_errors))
    }
    .await;
    browser.shutdown().await;
    result
}

// 采集流程见 state.rs 中的 collect_accounts。

/// 登录状态探针：打开普通抖音首页，只判断是否已登录。
pub async fn probe_login(
    exe: PathBuf,
    profile_dir: PathBuf,
    headless: bool,
    logger: &Logger,
) -> Result<(String, String)> {
    let browser = Browser::open(exe, profile_dir, headless, logger).await?;
    let result = async {
        browser
            .goto(DOUYIN_HOME, Duration::from_secs(4), Duration::from_secs(45))
            .await?;
        let value = browser
            .page
            .evaluate(crate::extractor_js::LOGIN_PROBE_SCRIPT)
            .await?;
        Ok::<_, anyhow::Error>(judge_login(&value))
    }
    .await;
    browser.shutdown().await;
    result
}

/// 在**已打开**的浏览器实例里就地判断登录状态：不新建标签页、不导航，
/// 因此不会打断用户正在扫码的登录页面。返回 None 表示当前没有可用的页面。
pub async fn probe_login_open_tab(
    profile_dir: &Path,
    logger: &Logger,
) -> Result<Option<(String, String)>> {
    let probe = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();
    let Some(ws_url) = cdp::live_debugger_url(profile_dir, &probe).await else {
        return Ok(None);
    };
    let client = CdpClient::connect(&ws_url).await?;
    let targets = client
        .send("Target.getTargets", serde_json::json!({}), None)
        .await?;
    let infos = targets
        .get("targetInfos")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let mut douyin_tab: Option<String> = None;
    let mut fallback: Option<String> = None;
    for info in &infos {
        if info.get("type").and_then(|value| value.as_str()) != Some("page") {
            continue;
        }
        let url = info.get("url").and_then(|value| value.as_str()).unwrap_or_default();
        let Some(target_id) = info.get("targetId").and_then(|value| value.as_str()) else {
            continue;
        };
        if url.contains("douyin.com") {
            douyin_tab = Some(target_id.to_string());
            break;
        }
        if fallback.is_none() && url.starts_with("http") {
            fallback = Some(target_id.to_string());
        }
    }
    let Some(target_id) = douyin_tab.or(fallback) else {
        return Ok(None);
    };
    let page = PageSession::attach_existing(&client, &target_id, logger).await?;
    let eval_res = page.evaluate(crate::extractor_js::LOGIN_PROBE_SCRIPT).await;
    page.close().await;
    let value = eval_res?;
    Ok(Some(judge_login(&value)))
}

/// 解析登录探针脚本的返回值。
fn judge_login(value: &serde_json::Value) -> (String, String) {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|item| item.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let flag = |key: &str| {
        value
            .get(key)
            .and_then(|item| item.as_bool())
            .unwrap_or(false)
    };
    let url = text("url");
    if flag("challenge") {
        (
            "challenge".to_string(),
            "页面要求安全验证，请在登录窗口中完成验证后重试".to_string(),
        )
    } else if flag("loggedOut") {
        (
            "expired".to_string(),
            "尚未登录抖音账号，请点击「登录窗口」扫码登录".to_string(),
        )
    } else if flag("loggedIn") {
        ("ok".to_string(), format!("已登录抖音账号（{url}）"))
    } else {
        ("error".to_string(), format!("无法确认登录状态，当前页面：{url}"))
    }
}
