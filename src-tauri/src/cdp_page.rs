use crate::cdp_client::CdpClient;
use crate::logging::Logger;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub struct PageSession {
    pub client: CdpClient,
    pub target_id: String,
    pub session_id: String,
    /// 该标签页是否由本应用创建（复用用户已打开的标签页时为 false，关闭时不应关掉它）
    created: bool,
}

impl PageSession {
    /// 新建标签页并附加 CDP 会话（flatten 模式，事件带 sessionId 返回）。
    pub async fn open(client: &CdpClient, logger: &Logger, viewport: bool) -> Result<Self> {
        let created = client
            .send("Target.createTarget", json!({ "url": "about:blank" }), None)
            .await?;
        let target_id = created
            .get("targetId")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("创建标签页失败：缺少 targetId"))?
            .to_string();

        let page = Self::attach(client, &target_id, true, logger, viewport).await?;
        logger.info("browser", format!("已创建页面会话 target={target_id}"));
        Ok(page)
    }

    /// 优先复用已经打开的标签页（例如用户自己打开的创作者中心），否则新建一个。
    pub async fn open_or_reuse(
        client: &CdpClient,
        prefer_url_contains: &str,
        logger: &Logger,
        viewport: bool,
    ) -> Result<Self> {
        if let Ok(targets) = client.send("Target.getTargets", json!({}), None).await {
            let infos = targets
                .get("targetInfos")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            let mut fallback: Option<String> = None;
            for info in &infos {
                if info.get("type").and_then(|value| value.as_str()) != Some("page") {
                    continue;
                }
                let url = info.get("url").and_then(|value| value.as_str()).unwrap_or_default();
                let target_id = info.get("targetId").and_then(|value| value.as_str());
                let Some(target_id) = target_id else { continue };
                if url.contains(prefer_url_contains) {
                    match Self::attach(client, target_id, false, logger, viewport).await {
                        Ok(page) => {
                            logger.info("browser", "复用已打开的创作者中心标签页");
                            return Ok(page);
                        }
                        Err(err) => logger.warn("browser", format!("复用标签页失败：{err:#}")),
                    }
                } else if fallback.is_none() && url.starts_with("http") {
                    fallback = Some(target_id.to_string());
                }
            }
            if let Some(target_id) = fallback {
                if let Ok(page) = Self::attach(client, &target_id, false, logger, viewport).await {
                    logger.info("browser", "复用已有标签页进行采集");
                    return Ok(page);
                }
            }
        }
        Self::open(client, logger, viewport).await
    }

    /// 附加到已存在的标签页（不新建、不改变视口），用于只读判断页面状态。
    pub async fn attach_existing(client: &CdpClient, target_id: &str, logger: &Logger) -> Result<Self> {
        Self::attach(client, target_id, false, logger, false).await
    }

    async fn attach(
        client: &CdpClient,
        target_id: &str,
        created: bool,
        _logger: &Logger,
        viewport: bool,
    ) -> Result<Self> {
        let attached = client
            .send(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
                None,
            )
            .await?;
        let session_id = attached
            .get("sessionId")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow!("附加标签页失败：缺少 sessionId"))?
            .to_string();

        let page = Self {
            client: client.clone(),
            target_id: target_id.to_string(),
            session_id,
            created,
        };
        let _ = page.send("Page.enable", json!({})).await;
        let _ = page.send("Runtime.enable", json!({})).await;
        let _ = page
            .send(
                "Network.enable",
                json!({ "maxTotalBufferSize": 100_000_000, "maxResourceBufferSize": 20_000_000 }),
            )
            .await;
        if viewport {
            let _ = page
                .send(
                    "Emulation.setDeviceMetricsOverride",
                    json!({ "width": 1440, "height": 960, "deviceScaleFactor": 1, "mobile": false }),
                )
                .await;
        }
        Ok(page)
    }

    /// 精准开启 Fetch 域拦截（仅用于采集视频详情时拦截 detail 接口，避免常规页面请求被挂起）
    pub async fn enable_fetch_interception(&self) -> Result<()> {
        self.send(
            "Fetch.enable",
            json!({
                "patterns": [
                    {
                        "urlPattern": "*aweme/v1/web/aweme/detail*",
                        "requestStage": "Response"
                    }
                ]
            }),
        )
        .await?;
        Ok(())
    }

    /// 采集完成或退出时关闭 Fetch 域拦截，放行所有请求
    pub async fn disable_fetch_interception(&self) -> Result<()> {
        let _ = self.send("Fetch.disable", json!({})).await;
        Ok(())
    }

    pub async fn send(&self, method: &str, params: Value) -> Result<Value> {
        self.client.send(method, params, Some(&self.session_id)).await
    }

    pub async fn evaluate(&self, expression: &str) -> Result<Value> {
        self.client.evaluate(&self.session_id, expression).await
    }

    pub async fn navigate(&self, url: &str) -> Result<()> {
        self.client
            .send(
                "Page.navigate",
                json!({ "url": url }),
                Some(&self.session_id),
            )
            .await?;
        Ok(())
    }

    pub async fn current_url(&self) -> Result<String> {
        Ok(self
            .evaluate("location.href")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    pub async fn title(&self) -> Result<String> {
        Ok(self
            .evaluate("document.title")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// 等待 document.readyState === complete
    pub async fn wait_ready(&self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let state = self
                .evaluate("document.readyState")
                .await
                .unwrap_or(Value::Null);
            if state.as_str() == Some("complete") {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(anyhow!("等待页面加载超时"));
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
    }

    /// 轮询 JS 条件表达式，直到返回真值或超时。
    pub async fn wait_for(&self, condition: &str, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let value = self.evaluate(condition).await.unwrap_or(Value::Null);
            match value {
                Value::Bool(true) => return Ok(()),
                Value::Number(num) if num.as_i64().unwrap_or(0) > 0 => return Ok(()),
                Value::Array(items) if !items.is_empty() => return Ok(()),
                _ => {}
            }
            if Instant::now() > deadline {
                return Err(anyhow!("等待页面元素超时"));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// 逐步滚动到底部，直到页面高度稳定（触发懒加载）。
    pub async fn scroll_to_bottom(&self, rounds: u32, pause: Duration) -> Result<u32> {
        let mut stable = 0;
        let mut performed = 0;
        for _ in 0..rounds {
            let before = self
                .evaluate("(() => { const el = document.scrollingElement || document.body; el.scrollTo(0, el.scrollHeight); return el.scrollHeight; })()")
                .await
                .unwrap_or(json!(0))
                .as_i64()
                .unwrap_or(0);
            performed += 1;
            tokio::time::sleep(pause).await;
            let after = self
                .evaluate("(() => { const el = document.scrollingElement || document.body; return el.scrollHeight; })()")
                .await
                .unwrap_or(json!(0))
                .as_i64()
                .unwrap_or(0);
            if after <= before {
                stable += 1;
                if stable >= 2 {
                    break;
                }
            } else {
                stable = 0;
            }
        }
        Ok(performed)
    }

    pub async fn close(&self) {
        if self.created {
            let _ = self
                .client
                .send(
                    "Target.closeTarget",
                    json!({ "targetId": self.target_id }),
                    None,
                )
                .await;
        } else {
            let _ = self
                .client
                .send(
                    "Target.detachFromTarget",
                    json!({ "sessionId": self.session_id }),
                    None,
                )
                .await;
        }
    }
}
