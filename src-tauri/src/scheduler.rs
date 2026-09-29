use crate::collect;
use crate::state::AppState;
use std::sync::Arc;
use std::time::Duration;

/// 后台调度：每 30 秒检查一次，按固定间隔（COLLECT_INTERVAL_SECS）触发采集，随后评估报警规则。
/// 只有「已登录 + 配置了监控视频」的账号会被自动采集。
pub fn spawn(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        // 等待窗口与前端事件通道就绪
        tokio::time::sleep(Duration::from_secs(10)).await;
        state.logger.info("scheduler", "后台采集调度已启动");
        loop {
            let paused = state
                .runtime
                .lock()
                .map(|runtime| runtime.paused)
                .unwrap_or(false);
            if !paused {
                let due = due_account_ids(&state);
                if !due.is_empty() {
                    state.logger.info("scheduler", format!("到期待采集账号：{due:?}"));
                    let outcomes = collect::collect_accounts(&state, due).await;
                    if outcomes.iter().any(|outcome| outcome.ok) {
                        if let Err(err) = collect::evaluate_and_notify(&state).await {
                            state
                                .logger
                                .error("scheduler", format!("规则评估失败：{err:#}"));
                        }
                    }
                }
            }
            update_next_run(&state);
            state.emit_status();
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}

fn due_account_ids(state: &AppState) -> Vec<i64> {
    let now = crate::util::now_ts();
    let accounts = match state.db(crate::repo::list_accounts) {
        Ok(accounts) => accounts,
        Err(_) => return Vec::new(),
    };
    let monitored = state
        .db(crate::repo_monitored::accounts_with_monitors)
        .unwrap_or_default();
    accounts
        .into_iter()
        .filter(|account| account.enabled)
        .filter(crate::collect::auto_collect_allowed)
        .filter(|account| monitored.contains(&account.id))
        .filter(|account| match account.last_collect_at {
            None => true,
            Some(ts) => now - ts >= crate::collect::COLLECT_INTERVAL_SECS,
        })
        .map(|account| account.id)
        .collect()
}

fn update_next_run(state: &AppState) {
    let accounts = match state.db(crate::repo::list_accounts) {
        Ok(accounts) => accounts,
        Err(_) => return,
    };
    let monitored = state
        .db(crate::repo_monitored::accounts_with_monitors)
        .unwrap_or_default();
    let next = accounts
        .iter()
        .filter(|account| account.enabled && monitored.contains(&account.id))
        .map(|account| account.last_collect_at.unwrap_or(0) + crate::collect::COLLECT_INTERVAL_SECS)
        .min();
    if let Ok(mut runtime) = state.runtime.lock() {
        runtime.next_run_at = next;
    }
}
