//! 登录观察：打开登录窗口后持续监控登录状态，扫码成功后立即更新账号状态并通知前端
//! （不需要用户手动点「检测」）。
//!
//! 注意：不能靠读取浏览器 Cookie 数据库来判断——抖音的会话 Cookie 多为
//! 「会话 Cookie」，Chrome 只保存在内存里、不落盘，因此这里直接探测页面本身。
//!
//! 探测策略：
//! 1. 登录窗口还开着：每 3 秒在**当前页面**上就地判断（不导航、不刷新，不打断扫码）；
//! 2. 窗口已关闭：改用无界面实例做完整探测，间隔按 30s → 60s → 120s → 300s 退避；
//! 3. 人工验证期间继续观察，仅确认已登录或账号删除时结束（不设总超时）。

use crate::collector;
use crate::state::{account_profile_dir, AppState};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// 登录窗口打开时的轮询间隔
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// 需要另起浏览器做完整探测时的首次间隔
const FULL_PROBE_FIRST: u64 = 30;
/// 完整探测的最大间隔（避免长期未登录时频繁启动浏览器）
const FULL_PROBE_MAX: u64 = 300;

/// 正在观察的账号（避免重复点「登录窗口」时重复轮询）。
fn watching() -> &'static Mutex<HashSet<i64>> {
    static WATCHING: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
    WATCHING.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn watch(state: Arc<AppState>, account_id: i64) {
    {
        let Ok(mut set) = watching().lock() else { return };
        if !set.insert(account_id) {
            return;
        }
    }
    tauri::async_runtime::spawn(async move {
        state.logger.info(
            "login",
            format!("开始持续监控账号 #{account_id} 的登录状态，扫码成功后会立即更新"),
        );
        let mut last_full_probe: Option<tokio::time::Instant> = None;
        let mut challenge_reported = false;
        let mut full_probe_gap = FULL_PROBE_FIRST;
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let account = match state.db(|conn| crate::repo::get_account(conn, account_id)) {
                Ok(Some(account)) => account,
                // 账号已被删除
                _ => break,
            };
            let profile_dir = account_profile_dir(&state, &account);

            // 1) 优先复用已打开的登录窗口，就地判断（不刷新页面、不新建标签页）
            let mut result = collector::probe_login_open_tab(&profile_dir, &state.logger).await;
            // 2) 窗口已关闭时，按退避间隔另起无界面实例确认
            if matches!(result, Ok(None)) {
                let due = last_full_probe
                    .map(|last| last.elapsed() >= Duration::from_secs(full_probe_gap))
                    .unwrap_or(true);
                if due {
                    last_full_probe = Some(tokio::time::Instant::now());
                    full_probe_gap = (full_probe_gap * 2).min(FULL_PROBE_MAX);
                    let settings = state.settings();
                    result = match state.browser(&settings) {
                        Ok(exe) => collector::probe_login(
                            exe,
                            profile_dir.clone(),
                            settings.headless,
                            &state.logger,
                        )
                        .await
                        .map(Some),
                        Err(err) => Err(err),
                    };
                }
            }

            match result {
                Ok(Some((login_state, message))) if login_state == "ok" => {
                    finish(&state, account_id, &login_state, &message);
                    break;
                }
                Ok(Some((login_state, message))) if login_state == "challenge" => {
                    if !challenge_reported {
                        finish(&state, account_id, &login_state, &message);
                        challenge_reported = true;
                    }
                }
                // 尚未登录（登录页 / 跳转中），继续等待
                Ok(_) => { challenge_reported = false; }
                Err(err) => state
                    .logger
                    .warn("login", format!("账号 #{account_id} 登录检测失败：{err:#}")),
            }
        }
        if let Ok(mut set) = watching().lock() {
            set.remove(&account_id);
        }
    });
}

fn finish(state: &AppState, account_id: i64, login_state: &str, message: &str) {
    let _ = state.db(|conn| {
        crate::repo::update_login_state(conn, account_id, login_state, message)
    });
    state.logger.info(
        "login",
        format!("账号 #{account_id} 登录检测：{login_state} {message}"),
    );
    state.emit(
        "login:changed",
        serde_json::json!({
            "account_id": account_id,
            "login_state": login_state,
            "message": message,
        }),
    );
    state.emit_status();
}
