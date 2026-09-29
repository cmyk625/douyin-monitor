use crate::models::{Account, Alert, CollectOutcome, ExtractPreview, RawWork};
use crate::state::{account_profile_dir, AppState};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 采集间隔固定为 10 分钟（写在代码里，界面与设置不再提供修改入口）。
pub const COLLECT_INTERVAL_SECS: i64 = 600;

/// 单账号采集的整体超时（包含启动浏览器、打开页面、解析与入库）。
const COLLECT_TASK_TIMEOUT: Duration = Duration::from_secs(300);

/// 采集指定账号（只采集账号监控名单里的视频），串行执行以避免同时打开多个浏览器。
pub async fn collect_accounts(state: &AppState, account_ids: Vec<i64>) -> Vec<CollectOutcome> {
    let settings = state.settings();
    let accounts = match state.db(crate::repo::list_accounts) {
        Ok(accounts) => accounts,
        Err(err) => {
            state.logger.error("collect", format!("读取账号失败：{err}"));
            return Vec::new();
        }
    };

    let targets: Vec<Account> = accounts
        .into_iter()
        .filter(|acc| account_ids.contains(&acc.id))
        .collect();

    if targets.is_empty() {
        state.logger.warn("collect", "没有可采集的账号（请确认账号已启用并完成登录）");
        return Vec::new();
    }

    {
        let mut runtime = match state.runtime.lock() {
            Ok(runtime) => runtime,
            Err(_) => return Vec::new(),
        };
        if runtime.collecting {
            state.logger.warn("collect", "已有采集任务在运行，跳过本次触发");
            return Vec::new();
        }
        runtime.collecting = true;
        runtime.last_error.clear();
    }
    state.emit_status();

    let mut outcomes = Vec::new();
    for account in targets {
        // 单个账号的采集整体限时，避免浏览器/网络异常时把采集任务永久卡住。
        let outcome = match tokio::time::timeout(
            COLLECT_TASK_TIMEOUT,
            collect_one(state, &settings, &account),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(_) => {
                let message = format!(
                    "采集超时（超过 {} 秒），已放弃本次采集",
                    COLLECT_TASK_TIMEOUT.as_secs()
                );
                state
                    .logger
                    .warn("collect", format!("「{}」{message}", account.name));
                let _ = state.db(|conn| {
                    crate::repo::update_login_state(conn, account.id, "error", &message)
                });
                CollectOutcome {
                    account_id: account.id,
                    account_name: account.name.clone(),
                    ok: false,
                    works_found: 0,
                    works_saved: 0,
                    new_works: 0,
                    snapshots: 0,
                    login_state: "error".into(),
                    message,
                    duration_ms: COLLECT_TASK_TIMEOUT.as_millis(),
                }
            }
        };
        {
            if let Ok(mut runtime) = state.runtime.lock() {
                runtime.current_account.clear();
            }
        }
        state.emit("collect:finished", outcome.clone());
        outcomes.push(outcome);
    }

    // 采集完成后执行自动滚动覆盖清理（若配置启用）
    if settings.auto_clean && settings.retention_days > 0 {
        let cutoff = crate::util::now_ts() - settings.retention_days * 86400;
        if let Ok(cleaned) = state.db(|conn| crate::repo_work::clean_expired_snapshots(conn, cutoff)) {
            if cleaned > 0 {
                state.logger.info(
                    "storage",
                    format!(
                        "自动滚动覆盖清理完成：已淘汰 {cleaned} 条超过 {} 天的历史快照",
                        settings.retention_days
                    ),
                );
            }
        }
    }

    {
        if let Ok(mut runtime) = state.runtime.lock() {
            runtime.collecting = false;
            runtime.current_account.clear();
            runtime.last_run_at = Some(crate::util::now_ts());
        }
    }
    state.emit_status();
    outcomes
}

async fn collect_one(
    state: &AppState,
    settings: &crate::settings::Settings,
    account: &Account,
) -> CollectOutcome {
    let started = Instant::now();
    let now = crate::util::now_ts();
    state.logger.info("collect", format!("开始采集「{}」", account.name));
    state.emit(
        "collect:started",
        serde_json::json!({ "account_id": account.id, "account_name": account.name }),
    );
    if let Ok(mut runtime) = state.runtime.lock() {
        runtime.current_account = account.name.clone();
    }
    state.emit_status();

    let mut outcome = CollectOutcome {
        account_id: account.id,
        account_name: account.name.clone(),
        ok: false,
        works_found: 0,
        works_saved: 0,
        new_works: 0,
        snapshots: 0,
        login_state: account.login_state.clone(),
        message: String::new(),
        duration_ms: 0,
    };

    // 只采集监控名单里的视频：没有配置就不启动浏览器。
    let tracked = match state.db(|conn| crate::repo_monitored::enabled_aweme_ids(conn, account.id)) {
        Ok(ids) => ids,
        Err(err) => {
            outcome.message = format!("读取监控名单失败：{err}");
            outcome.login_state = "error".into();
            outcome.duration_ms = started.elapsed().as_millis();
            return outcome;
        }
    };
    if tracked.is_empty() {
        outcome.message =
            "该账号还没有配置监控视频：请到「作品」页点击「新增监控视频」添加后再采集".into();
        state
            .logger
            .warn("collect", format!("「{}」{}", account.name, outcome.message));
        outcome.duration_ms = started.elapsed().as_millis();
        return outcome;
    }

    let exe = match state.browser(settings) {
        Ok(exe) => exe,
        Err(err) => {
            outcome.message = err.to_string();
            outcome.login_state = "error".into();
            let _ = state.db(|conn| crate::repo::update_login_state(conn, account.id, "error", &outcome.message));
            outcome.duration_ms = started.elapsed().as_millis();
            return outcome;
        }
    };
    let profile_dir = account_profile_dir(state, account);

    let fetched = crate::collector::fetch_works(
        exe,
        profile_dir,
        settings,
        &tracked,
        &state.logger,
    )
    .await;

    match fetched {
        Err(err) => {
            let message = format!("{err:#}");
            outcome.message = message.clone();
            outcome.login_state = if message.contains("登录") { "expired".into() } else { "error".into() };
            let _ = state.db(|conn| {
                crate::repo::update_login_state(conn, account.id, &outcome.login_state, &message)?;
                crate::repo::mark_collected(conn, account.id, now, false, &message)
            });
        }
        Ok((works, preview, item_errors)) => {
            outcome.works_found = works.len();
            if preview.blocked {
                // 页面被登录墙/验证码拦截
                outcome.login_state = if preview.reason.contains("验证") {
                    "challenge".into()
                } else {
                    "expired".into()
                };
                outcome.message = if preview.reason.is_empty() {
                    "页面要求安全验证，请在登录窗口中完成验证后重试".to_string()
                } else {
                    preview.reason.clone()
                };
                let _ = state.db(|conn| {
                    crate::repo::update_login_state(conn, account.id, &outcome.login_state, &outcome.message)?;
                    for id in &tracked {
                        let err = item_errors.get(id).unwrap_or(&outcome.message);
                        crate::repo_monitored::mark_missing(conn, account.id, id, err)?;
                    }
                    crate::repo::mark_collected(conn, account.id, now, false, &outcome.message)
                });
                state.logger.warn("collect", format!("「{}」{}", account.name, outcome.message));
            } else if works.is_empty() {
                // 页面正常但没解析到任何作品（可能全部下架、私密或单视频解析失败），
                // 此时登录状态保持不变（避免单视频失败污染账号整体登录状态）。
                outcome.login_state = account.login_state.clone();
                outcome.message = format!("监控的 {} 条视频均未能成功读取数据", tracked.len());
                let _ = state.db(|conn| {
                    for id in &tracked {
                        let err = item_errors.get(id).cloned().unwrap_or_else(|| "视频页未返回可用指标".to_string());
                        crate::repo_monitored::mark_missing(conn, account.id, id, &err)?;
                    }
                    crate::repo::mark_collected(conn, account.id, now, false, &outcome.message)
                });
                state.logger.warn("collect", format!("「{}」{}", account.name, outcome.message));
            } else {
                match save_works(state, account, &tracked, &works, &item_errors, now) {
                    Ok(result) => {
                        outcome.works_saved = result.saved;
                        outcome.new_works = result.new_works;
                        outcome.snapshots = result.snapshots;
                        outcome.ok = true;
                        outcome.login_state = "ok".into();
                        let mut message = format!(
                            "监控 {} 条，命中 {} 条，入库 {} 条，新增 {} 条，快照 {} 条",
                            result.tracked,
                            result.hit,
                            result.saved,
                            result.new_works,
                            result.snapshots
                        );
                        if !result.missing.is_empty() {
                            message.push_str(&format!("；另有 {} 条未成功读取", result.missing.len()));
                        }
                        outcome.message = message;
                        state
                            .logger
                            .info("collect", format!("「{}」{}", account.name, outcome.message));
                        let _ = state.db(|conn| {
                            crate::repo::update_login_state(conn, account.id, "ok", "")?;
                            crate::repo::mark_collected(conn, account.id, now, true, "")
                        });
                    }
                    Err(err) => {
                        outcome.message = format!("写入数据库失败：{err}");
                        outcome.login_state = "error".into();
                        let _ = state.db(|conn| {
                            crate::repo::update_login_state(conn, account.id, "error", &outcome.message)?;
                            crate::repo::mark_collected(conn, account.id, now, false, &outcome.message)
                        });
                    }
                }
            }
        }
    }

    outcome.duration_ms = started.elapsed().as_millis();
    outcome
}

/// 采集结果统计（只统计监控名单内的视频）。
struct MonitorSave {
    tracked: usize,
    hit: usize,
    saved: usize,
    new_works: usize,
    snapshots: usize,
    missing: Vec<String>,
}

/// 只把监控名单里的作品写入 works / snapshots，其余解析结果一律忽略。
fn save_works(
    state: &AppState,
    account: &Account,
    tracked: &[String],
    works: &[RawWork],
    item_errors: &HashMap<String, String>,
    now: i64,
) -> Result<MonitorSave> {
    state.db(|conn| {
        conn.execute_batch("BEGIN TRANSACTION;")?;
        let res: Result<MonitorSave> = (|| {
            let mut result = MonitorSave {
                tracked: tracked.len(),
                hit: 0,
                saved: 0,
                new_works: 0,
                snapshots: 0,
                missing: Vec::new(),
            };
            let mut found: Vec<String> = Vec::new();
            for raw in works {
                let aweme_id = raw.aweme_id.trim();
                if aweme_id.is_empty() || !tracked.iter().any(|id| id == aweme_id) {
                    continue;
                }
                let change = crate::repo_work::upsert_work(conn, account.id, raw, now)?;
                result.hit += 1;
                result.saved += 1;
                if change.is_new {
                    result.new_works += 1;
                }
                if change.changed {
                    result.snapshots += 1;
                }
                crate::repo_monitored::mark_seen(conn, account.id, aweme_id, &raw.title, &raw.author_name, &raw.url, now)?;
                found.push(aweme_id.to_string());
            }
            for id in tracked {
                if !found.iter().any(|item| item == id) {
                    let err_msg = item_errors
                        .get(id)
                        .map(|s| s.as_str())
                        .unwrap_or("视频页未返回可用指标，请确认视频可访问；页面可能需要登录或结构已变化");
                    crate::repo_monitored::mark_missing(
                        conn,
                        account.id,
                        id,
                        err_msg,
                    )?;
                    result.missing.push(id.clone());
                }
            }
            Ok(result)
        })();

        match res {
            Ok(result) => {
                conn.execute_batch("COMMIT;")?;
                Ok(result)
            }
            Err(err) => {
                let _ = conn.execute_batch("ROLLBACK;");
                Err(err)
            }
        }
    })
}

/// 试采集：解析并返回预览，不写库。
pub async fn preview_account(state: &AppState, account_id: i64) -> Result<ExtractPreview> {
    let settings = state.settings();
    let account = state
        .db(|conn| crate::repo::get_account(conn, account_id))?
        .ok_or_else(|| anyhow!("账号不存在"))?;
    let exe = state.browser(&settings)?;
    let profile_dir = account_profile_dir(state, &account);
    let tracked = state.db(|conn| crate::repo_monitored::enabled_aweme_ids(conn, account_id))?;
    crate::collector::fetch_works(
        exe,
        profile_dir,
        &settings,
        &tracked,
        &state.logger,
    )
    .await.map(|(_, preview, _)| preview)
}

/// 是否允许自动（定时 / 批量）采集：
///
/// - `ok` 已登录、`error` 采集异常（可能是网络或页面结构问题）→ 允许，便于自动恢复；
/// - `unknown` 尚未确认登录 → 跳过，避免与用户正在扫码的登录窗口抢同一个浏览器目录；
/// - `expired` / `challenge` 登录失效或需要人工验证 → 跳过，避免反复打开浏览器。
///
/// 单账号手动「采集」不受此限制。
pub fn auto_collect_allowed(account: &Account) -> bool {
    matches!(account.login_state.as_str(), "ok" | "error")
}

/// 检测登录状态（打开抖音首页，只判断是否已登录，不要求解析出作品）。
pub async fn check_login(state: &AppState, account_id: i64) -> Result<(String, String)> {
    let settings = state.settings();
    let account = state
        .db(|conn| crate::repo::get_account(conn, account_id))?
        .ok_or_else(|| anyhow!("账号不存在"))?;
    let exe = state.browser(&settings)?;
    let profile_dir = account_profile_dir(state, &account);
    let (login_state, message) = crate::collector::probe_login(
        exe,
        profile_dir,
        settings.headless,
        &state.logger,
    )
    .await?;
    state.db(|conn| crate::repo::update_login_state(conn, account_id, &login_state, &message))?;
    state.logger.info(
        "login",
        format!("账号 #{account_id} 登录检测：{login_state} {message}"),
    );
    Ok((login_state, message))
}

/// 采集结束后评估规则并推送飞书。
pub async fn evaluate_and_notify(state: &AppState) -> Result<Vec<Alert>> {
    let now = crate::util::now_ts();
    let alerts = state.db(|conn| crate::rules::evaluate(conn, now))?;
    if alerts.is_empty() {
        return Ok(alerts);
    }

    let feishu_cfg = crate::config::AppConfig::load().feishu;
    for alert in &alerts {
        if !feishu_cfg.enabled {
            let _ = state.db(|conn| crate::repo_alert::mark_notify(conn, alert.id, "skipped", "未启用飞书通知"));
            continue;
        }
        let result = crate::feishu::send_text(
            &state.http,
            &feishu_cfg.webhook,
            &feishu_cfg.secret,
            &alert.message,
        )
        .await;
        match result {
            Ok(response) if response.ok => {
                state.logger.info("feishu", format!("报警 #{} 已推送飞书", alert.id));
                let _ = state.db(|conn| crate::repo_alert::mark_notify(conn, alert.id, "sent", "success"));
            }
            Ok(response) => {
                let detail = format!("code={} {}", response.code, response.msg);
                state.logger.error("feishu", format!("报警 #{} 推送失败：{detail}", alert.id));
                let _ = state.db(|conn| crate::repo_alert::mark_notify(conn, alert.id, "failed", &detail));
            }
            Err(err) => {
                let detail = format!("{err:#}");
                state.logger.error("feishu", format!("报警 #{} 推送异常：{detail}", alert.id));
                let _ = state.db(|conn| crate::repo_alert::mark_notify(conn, alert.id, "failed", &detail));
            }
        }
    }

    state.emit("alert:created", alerts.clone());
    state.emit_status();
    Ok(alerts)
}
