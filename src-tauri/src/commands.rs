use crate::collect;
use crate::models::{
    Account, AccountInput, Alert, CollectOutcome, ExtractPreview, LoginCheck, LogEntry, MonitoredInput,
    MonitoredWork, NotifyResult, Rule, RuleInput, RuntimeStatus, SnapshotPoint, Work,
};
use crate::settings::Settings;
use crate::state::AppState;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_accounts(state: State<'_, Arc<AppState>>) -> Result<Vec<Account>, String> {
    state
        .db(crate::repo::list_accounts)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn create_account(state: State<'_, Arc<AppState>>, input: AccountInput) -> Result<Account, String> {
    if input.name.trim().is_empty() {
        return Err("账号名称不能为空".into());
    }
    let id = state
        .db(|conn| crate::repo::create_account(conn, &input, "", crate::util::now_ts()))
        .map_err(|err| err.to_string())?;
    let profile = state.profile_dir(id, &input.name);
    state
        .db(|conn| {
            crate::repo::set_profile_dir(conn, id, &profile.display().to_string())?;
            Ok(())
        })
        .map_err(|err| err.to_string())?;
    state
        .db(|conn| crate::repo::get_account(conn, id))
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "账号创建失败".to_string())
}

#[tauri::command]
pub fn update_account(state: State<'_, Arc<AppState>>, input: crate::models::AccountRename) -> Result<Account, String> {
    let id = input.id;
    state
        .db(|conn| crate::repo::update_account(conn, &input))
        .map_err(|err| err.to_string())?;
    state.emit_status();
    state
        .db(|conn| crate::repo::get_account(conn, id))
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "账号不存在".to_string())
}

#[tauri::command]
pub fn delete_account(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    let account = state
        .db(|conn| crate::repo::get_account(conn, id))
        .map_err(|err| err.to_string())?;

    state
        .db(|conn| crate::repo::delete_account(conn, id))
        .map_err(|err| err.to_string())?;

    if let Some(account) = account {
        let profile_dir = crate::state::account_profile_dir(&state, &account);
        if profile_dir.exists() {
            if let Err(err) = std::fs::remove_dir_all(&profile_dir) {
                state.logger.warn(
                    "account",
                    format!(
                        "清理账号「{}」浏览器缓存目录失败（可能被占用）：{err}",
                        account.name
                    ),
                );
            } else {
                state.logger.info(
                    "account",
                    format!("已清理账号「{}」的浏览器缓存目录", account.name),
                );
            }
        }
    }
    state.emit_status();
    Ok(())
}

#[tauri::command]
pub fn open_login_window(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    let account = state
        .db(|conn| crate::repo::get_account(conn, id))
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;
    let profile_dir = crate::state::account_profile_dir(&state, &account);
    state
        .open_browser_window(&profile_dir, crate::collector::DOUYIN_HOME)
        .map_err(|err| err.to_string())?;
    state.logger.info(
        "login",
        format!(
            "已为「{}」打开登录窗口（{}），扫码完成后应用会自动刷新登录状态",
            account.name,
            profile_dir.display()
        ),
    );
    // 后台观察登录状态：扫码成功后自动更新账号列表，无需手动点「检测」。
    crate::login_watch::watch(state.inner().clone(), id);
    Ok(())
}

#[tauri::command]
pub async fn check_login(state: State<'_, Arc<AppState>>, id: i64) -> Result<LoginCheck, String> {
    let (login_state, message) = collect::check_login(&state, id)
        .await
        .map_err(|err| err.to_string())?;
    state.emit_status();
    Ok(LoginCheck { login_state, message })
}

#[tauri::command]
pub async fn collect_now(state: State<'_, Arc<AppState>>, id: i64) -> Result<Vec<CollectOutcome>, String> {
    let settings = state.settings();
    state.browser(&settings).map_err(|err| err.to_string())?;
    let outcomes = collect::collect_accounts(&state, vec![id]).await;
    if outcomes.is_empty() {
        return Err("没有可采集的账号：请确认该账号已启用".into());
    }
    if outcomes.iter().any(|outcome| outcome.ok) {
        let _ = collect::evaluate_and_notify(&state).await;
    }
    Ok(outcomes)
}

#[tauri::command]
pub async fn collect_all(state: State<'_, Arc<AppState>>) -> Result<Vec<CollectOutcome>, String> {
    let settings = state.settings();
    state.browser(&settings).map_err(|err| err.to_string())?;
    let accounts = state
        .db(crate::repo::list_accounts)
        .map_err(|err| err.to_string())?;
    let ids: Vec<i64> = accounts.into_iter().filter(|a| a.enabled).map(|a| a.id).collect();
    if ids.is_empty() {
        return Err("当前没有启用的采集账号：请先在「账号」页添加并启用账号".into());
    }
    let outcomes = collect::collect_accounts(&state, ids).await;
    if outcomes.iter().any(|outcome| outcome.ok) {
        let _ = collect::evaluate_and_notify(&state).await;
    }
    Ok(outcomes)
}

#[tauri::command]
pub fn recheck_chrome(state: State<'_, Arc<AppState>>) -> Result<Option<String>, String> {
    let settings = state.settings();
    let found = crate::cdp::find_browser(&settings.chrome_path);
    if let Ok(mut cache) = state.browser_path.lock() {
        *cache = found.clone();
    }
    state.emit_status();
    Ok(found.map(|p| p.display().to_string()))
}

#[tauri::command]
pub async fn preview_extract(state: State<'_, Arc<AppState>>, id: i64) -> Result<ExtractPreview, String> {
    collect::preview_account(&state, id).await.map_err(|err| format!("{err:#}"))
}

#[tauri::command]
pub fn list_monitored(state: State<'_, Arc<AppState>>) -> Result<Vec<MonitoredWork>, String> {
    state
        .db(crate::repo_monitored::list_monitored)
        .map_err(|err| err.to_string())
}

/// 新增 / 修改监控视频（负责人是手填的名字，会替换进报警内容）。
#[tauri::command]
pub async fn save_monitored(
    state: State<'_, Arc<AppState>>,
    input: MonitoredInput,
) -> Result<MonitoredWork, String> {
    let resolved = crate::repo_monitored::resolve_target(&state.http, &input.target)
        .await
        .map_err(|err| err.to_string())?;
    let id = state
        .db(|conn| crate::repo_monitored::save_monitored(conn, &input, Some(resolved), crate::util::now_ts()))
        .map_err(|err| err.to_string())?;
    state.emit_status();
    state
        .db(|conn| crate::repo_monitored::get_monitored(conn, id))
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "监控视频保存失败".to_string())
}

#[tauri::command]
pub fn delete_monitored(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    state
        .db(|conn| crate::repo_monitored::delete_monitored(conn, id))
        .map_err(|err| err.to_string())?;
    state.emit_status();
    Ok(())
}

#[tauri::command]
pub fn list_works(
    state: State<'_, Arc<AppState>>,
    account_id: Option<i64>,
    keyword: String,
    limit: i64,
    history: Option<bool>,
    offset: Option<i64>,
) -> Result<Vec<Work>, String> {
    state
        .db(|conn| crate::repo_work::list_work_page(conn, account_id, &keyword, limit.clamp(1, 2000), history.unwrap_or(false), offset.unwrap_or(0).max(0)))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn work_trend(
    state: State<'_, Arc<AppState>>,
    work_id: i64,
    limit: i64,
) -> Result<Vec<SnapshotPoint>, String> {
    state
        .db(|conn| crate::repo_work::work_trend(conn, work_id, limit.clamp(1, 5000)))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_work(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    state
        .db(|conn| crate::repo_work::delete_work(conn, id))
        .map_err(|err| err.to_string())?;
    state.emit_status();
    Ok(())
}

#[tauri::command]
pub fn list_rules(state: State<'_, Arc<AppState>>) -> Result<Vec<Rule>, String> {
    state
        .db(crate::repo_alert::list_rules)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_rule(state: State<'_, Arc<AppState>>, input: RuleInput) -> Result<Rule, String> {
    let now = crate::util::now_ts();
    let id = state
        .db(|conn| crate::repo_alert::save_rule(conn, &input, now))
        .map_err(|err| err.to_string())?;
    state
        .db(crate::repo_alert::list_rules)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| "规则保存失败".to_string())
}

#[tauri::command]
pub fn delete_rule(state: State<'_, Arc<AppState>>, id: i64) -> Result<(), String> {
    state
        .db(|conn| crate::repo_alert::delete_rule(conn, id))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn run_rules(state: State<'_, Arc<AppState>>) -> Result<usize, String> {
    let alerts = collect::evaluate_and_notify(&state)
        .await
        .map_err(|err| format!("{err:#}"))?;
    Ok(alerts.len())
}

#[tauri::command]
pub fn list_alerts(state: State<'_, Arc<AppState>>, limit: i64) -> Result<Vec<Alert>, String> {
    state
        .db(|conn| crate::repo_alert::list_alerts(conn, limit.clamp(1, 2000)))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn clear_alerts(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .db(crate::repo_alert::clear_alerts)
        .map_err(|err| err.to_string())?;
    state.emit_status();
    Ok(())
}

#[tauri::command]
pub async fn resend_alert(state: State<'_, Arc<AppState>>, id: i64) -> Result<NotifyResult, String> {
    let alert = state
        .db(|conn| crate::repo_alert::get_alert(conn, id))
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "报警记录不存在".to_string())?;
    let feishu_cfg = crate::config::AppConfig::load().feishu;
    let result = crate::feishu::send_text(
        &state.http,
        &feishu_cfg.webhook,
        &feishu_cfg.secret,
        &alert.message,
    )
    .await
    .map_err(|err| format!("{err:#}"))?;
    let (state_text, detail) = if result.ok {
        ("sent", "manual resend".to_string())
    } else {
        ("failed", format!("code={} {}", result.code, result.msg))
    };
    let _ = state.db(|conn| crate::repo_alert::mark_notify(conn, id, state_text, &detail));
    state.emit("notify:result", result.clone());
    Ok(result)
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn save_settings(state: State<'_, Arc<AppState>>, input: Settings) -> Result<Settings, String> {
    let mut settings = input;
    settings.clamp();
    // 设置页里改了「开机自启动」时同步到系统（即时生效，刷新注册表路径）。
    let previous = state.settings();
    if settings.autostart != previous.autostart {
        if let Some(enabled) = settings.autostart {
            apply_autostart(&state, enabled)?;
        }
    }
    state
        .save_settings(&settings)
        .map_err(|err| err.to_string())?;
    if let Ok(mut cache) = state.browser_path.lock() {
        *cache = None;
    }
    state.logger.info("settings", "设置已更新");
    state.emit_status();
    Ok(settings)
}

/// 立即开启/关闭开机自启动（写入或删除系统启动项）。
#[tauri::command]
pub fn set_autostart(state: State<'_, Arc<AppState>>, enabled: bool) -> Result<bool, String> {
    apply_autostart(&state, enabled)?;
    let mut settings = state.settings();
    settings.autostart = Some(enabled);
    state
        .save_settings(&settings)
        .map_err(|err| err.to_string())?;
    state.logger.info(
        "autostart",
        if enabled { "已开启开机自启动" } else { "已关闭开机自启动" },
    );
    state.emit_status();
    Ok(enabled)
}

fn apply_autostart(state: &State<'_, Arc<AppState>>, enabled: bool) -> Result<(), String> {
    let handle = state
        .app_handle()
        .ok_or_else(|| "应用尚未就绪，请稍后重试".to_string())?;
    let result = if enabled {
        crate::autostart::enable(&handle)
    } else {
        crate::autostart::disable(&handle)
    };
    result.map_err(|err| format!("{err:#}"))
}

#[tauri::command]
pub fn get_feishu_config() -> crate::config::FeishuConfig {
    crate::config::AppConfig::load().feishu
}

#[tauri::command]
pub fn save_feishu_config(
    state: State<'_, Arc<AppState>>,
    input: crate::config::FeishuConfig,
) -> Result<crate::config::FeishuConfig, String> {
    let mut config = crate::config::AppConfig::load();
    config.feishu = crate::config::FeishuConfig {
        enabled: input.enabled,
        webhook: input.webhook.trim().to_string(),
        secret: input.secret.trim().to_string(),
        keyword: input.keyword.trim().to_string(),
    };
    let path = crate::config::AppConfig::config_path();
    config
        .save_to_path(&path)
        .map_err(|err| format!("保存飞书配置失败：{err}"))?;

    state.logger.info("feishu", "飞书机器人通知配置已更新");
    state.emit_status();
    Ok(config.feishu)
}

#[tauri::command]
pub async fn test_feishu(
    state: State<'_, Arc<AppState>>,
    config: Option<crate::config::FeishuConfig>,
) -> Result<NotifyResult, String> {
    let feishu_cfg = config.unwrap_or_else(|| crate::config::AppConfig::load().feishu);
    if !crate::feishu::is_configured(&feishu_cfg.webhook) {
        return Err("Webhook 地址无效，请填写正确的飞书自定义机器人 Webhook 地址".into());
    }
    let text = format!(
        "{}抖音作品监控：测试消息，通知链路正常。\n时间：{}",
        if feishu_cfg.keyword.trim().is_empty() {
            String::new()
        } else {
            format!("【{}】", feishu_cfg.keyword.trim())
        },
        crate::util::fmt_ts(crate::util::now_ts())
    );
    let result = crate::feishu::send_text(
        &state.http,
        &feishu_cfg.webhook,
        &feishu_cfg.secret,
        &text,
    )
    .await
    .map_err(|err| format!("{err:#}"))?;
    state.emit("notify:result", result.clone());
    Ok(result)
}

#[tauri::command]
pub fn get_status(state: State<'_, Arc<AppState>>) -> RuntimeStatus {
    state.status()
}

#[tauri::command]
pub fn set_paused(state: State<'_, Arc<AppState>>, paused: bool) -> Result<RuntimeStatus, String> {
    {
        let mut runtime = state.runtime.lock().map_err(|_| "状态锁失效".to_string())?;
        runtime.paused = paused;
    }
    state
        .logger
        .info("scheduler", if paused { "已暂停自动采集" } else { "已恢复自动采集" });
    state.emit_status();
    Ok(state.status())
}

#[tauri::command]
pub fn list_logs(state: State<'_, Arc<AppState>>, limit: i64) -> Vec<LogEntry> {
    state.logger.recent(limit.clamp(1, 1000) as usize)
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("http") {
        return Err("只允许打开 http/https 链接".into());
    }
    opener::open(&url).map_err(|err| anyhow!("打开链接失败：{err}").to_string())
}

#[tauri::command]
pub fn open_data_dir(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let dir = state.data_dir.clone();
    state.open_path(&dir).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn open_config_file(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let path = crate::config::AppConfig::config_path();
    if !path.exists() {
        let _ = crate::config::AppConfig::load();
    }
    state.open_path(&path).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_storage_stats(state: State<'_, Arc<AppState>>) -> Result<crate::models::StorageStats, String> {
    let settings = state.settings();
    let db_path = state.data_dir.join("douyin-monitor.db");
    state
        .db(|conn| crate::repo_work::get_storage_stats(conn, &db_path, &settings))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn clean_storage(
    state: State<'_, Arc<AppState>>,
    retention_days: Option<i64>,
) -> Result<crate::models::CleanStorageResult, String> {
    let settings = state.settings();
    let days = retention_days.unwrap_or(settings.retention_days).clamp(1, 365);
    let db_path = state.data_dir.join("douyin-monitor.db");
    let result = state
        .db(|conn| crate::repo_work::clean_storage(conn, &db_path, days))
        .map_err(|err| err.to_string())?;

    state.logger.info(
        "storage",
        format!(
            "覆盖清理完成：已淘汰 {} 条超期快照、{} 个孤立作品，磁盘空间释放 {} 字节",
            result.deleted_snapshots, result.deleted_works, result.freed_bytes
        ),
    );
    state.emit_status();
    Ok(result)
}

#[tauri::command]
pub async fn check_environment(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::models::EnvironmentReport, String> {
    Ok(crate::env_check::run_full_check(&state).await)
}
