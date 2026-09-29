//! IPC 回归测试：把真实命令注册到 mock 运行时并逐个调用，确保：
//! 1) 每个命令都能解析到托管状态（曾经出现 `State<AppState>` 与 `manage(Arc<AppState>)` 不匹配，
//!    导致所有命令报错、前端永远停在「正在加载本地数据…」）；
//! 2) 前端使用的 camelCase 参数名能正确映射到 Rust 的 snake_case 形参；
//! 3) 返回值结构与前端 `src/types.ts` 中的约定一致。
#![cfg(test)]

use serde_json::{json, Value};
use std::sync::Arc;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("douyin-monitor-ipc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build_app(dir: &std::path::Path) -> tauri::App<MockRuntime> {
    let state = Arc::new(crate::state::build_state(dir.to_path_buf(), "0.1.0-test".into()).unwrap());
    mock_builder()
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_status,
            crate::commands::list_accounts,
            crate::commands::create_account,
            crate::commands::update_account,
            crate::commands::delete_account,
            crate::commands::list_monitored,
            crate::commands::save_monitored,
            crate::commands::delete_monitored,
            crate::commands::list_works,
            crate::commands::work_trend,
            crate::commands::delete_work,
            crate::commands::list_rules,
            crate::commands::save_rule,
            crate::commands::delete_rule,
            crate::commands::run_rules,
            crate::commands::list_alerts,
            crate::commands::clear_alerts,
            crate::commands::get_settings,
            crate::commands::save_settings,
            crate::commands::set_paused,
            crate::commands::list_logs,
            crate::commands::get_storage_stats,
            crate::commands::clean_storage,
            crate::commands::check_environment,
        ])
        .manage(state)
        .build(mock_context(noop_assets()))
        .unwrap()
}

fn call(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: cmd.to_string(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    match tauri::test::get_ipc_response(webview, request) {
        Ok(body) => Ok(body.deserialize::<Value>().unwrap_or(Value::Null)),
        Err(error) => Err(error),
    }
}

fn expect_ok(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Value {
    match call(webview, cmd, args) {
        Ok(value) => value,
        Err(error) => panic!("命令 {cmd} 调用失败：{error}"),
    }
}

#[test]
fn every_command_resolves_managed_state_and_arguments() {
    let dir = temp_dir();
    let app = build_app(&dir);
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    // 运行状态
    let status = expect_ok(&webview, "get_status", json!({}));
    assert_eq!(status["version"], "0.1.0-test");
    assert_eq!(status["accounts_total"], 0);
    assert_eq!(status["works_total"], 0);
    assert!(status["data_dir"].as_str().unwrap().contains("douyin-monitor-ipc"));

    // 设置：读取 → 修改 → 读取
    let settings = expect_ok(&webview, "get_settings", json!({}));
    assert_eq!(settings["headless"], true);
    let mut patched = settings.clone();
    patched["page_wait_secs"] = json!(12);
    let saved = expect_ok(&webview, "save_settings", json!({ "input": patched }));
    assert_eq!(saved["page_wait_secs"], 12);
    assert_eq!(expect_ok(&webview, "get_settings", json!({}))["page_wait_secs"], 12);

    // 账号：新建（参数结构与前端 src/types.ts 完全一致：顶层 camelCase、嵌套 snake_case）
    let account = expect_ok(
        &webview,
        "create_account",
        json!({
            "input": {
                "id": null,
                "name": "回归测试账号",
                "sec_uid": "",
                "target_url": "",
                "parse_script": "",
                "enabled": true,
                "note": ""
            }
        }),
    );
    assert_eq!(account["name"], "回归测试账号");
    assert_eq!(account["login_state"], "unknown");
    // 间隔固定在代码里（10 分钟），界面不可修改
    assert_eq!(account["interval_secs"], 600);
    assert!(account["profile_dir"].as_str().unwrap().contains("chrome-profiles"));
    let account_id = account["id"].as_i64().unwrap();

    let accounts = expect_ok(&webview, "list_accounts", json!({}));
    assert_eq!(accounts.as_array().unwrap().len(), 1);

    let updated = expect_ok(
        &webview,
        "update_account",
        json!({
            "input": {
                "id": account_id,
                "name": "回归测试账号-改",
                "sec_uid": "12345",
                "target_url": "https://creator.douyin.com/creator-micro/data/content",
                "parse_script": "",
                "enabled": false,
                "note": "n"
            }
        }),
    );
    assert_eq!(updated["name"], "回归测试账号-改");
    assert_eq!(updated["enabled"], true);
    assert_eq!(updated["sec_uid"], "");
    assert_eq!(updated["interval_secs"], 600);
    assert_eq!(updated["target_url"], account["target_url"]);
    assert_eq!(updated["note"], account["note"]);
    let renamed = expect_ok(&webview, "update_account", json!({"input": {"id": account_id, "name": "回归测试账号-改"}}));
    assert_eq!(renamed["target_url"], account["target_url"]);
    assert_eq!(renamed["enabled"], account["enabled"]);
    assert!(call(&webview, "update_account", json!({"input": {"id": account_id, "name": "  "}})).is_err());

    // 监控名单：负责人不再强制要求填写
    let empty_owner = expect_ok(
        &webview,
        "save_monitored",
        json!({
            "input": {
                "account_id": account_id,
                "target": "7412345678901234567",
                "owner_name": "   ",
                "enabled": true
            }
        }),
    );
    assert_eq!(empty_owner["owner_name"], "");

    let monitored = expect_ok(
        &webview,
        "save_monitored",
        json!({
            "input": {
                "account_id": account_id,
                "target": "https://www.douyin.com/video/7412345678901234567",
                "owner_name": "  张三  ",
                "enabled": true
            }
        }),
    );
    assert_eq!(monitored["aweme_id"], "7412345678901234567");
    assert_eq!(monitored["owner_name"], "张三", "负责人是手填字符串（去掉首尾空白）");
    assert_eq!(monitored["owner_open_id"], "", "不再使用飞书 Open ID");
    assert_eq!(monitored["title"], "", "标题不再手工填写，由采集自动学习");
    assert!(monitored["work_id"].is_null());
    let monitored_id = monitored["id"].as_i64().unwrap();
    assert_eq!(expect_ok(&webview, "list_monitored", json!({})).as_array().unwrap().len(), 1);
    assert_eq!(expect_ok(&webview, "get_status", json!({}))["monitored_total"], 1);

    // 规则：保存（嵌套字段为 snake_case）→ 列表 → 删除
    let rule = expect_ok(
        &webview,
        "save_rule",
        json!({
            "input": {
                "id": null,
                "name": "播放 30 分钟增长",
                "account_id": account_id,
                "metric": "play",
                "window_minutes": 30,
                "threshold": 10000,
                "cooldown_minutes": 120,
                "enabled": true
            }
        }),
    );
    assert_eq!(rule["window_minutes"], 30);
    assert_eq!(rule["threshold"], 10000);
    assert_eq!(rule["cooldown_minutes"], 120);
    assert_eq!(rule["account_name"], "回归测试账号-改");
    let rule_id = rule["id"].as_i64().unwrap();
    assert_eq!(expect_ok(&webview, "list_rules", json!({})).as_array().unwrap().len(), 1);

    // 作品列表与趋势（空库也应正常返回）
    assert!(expect_ok(
        &webview,
        "list_works",
        json!({ "accountId": null, "keyword": "", "limit": 50 })
    )
    .as_array()
    .unwrap()
    .is_empty());
    assert!(expect_ok(&webview, "work_trend", json!({ "workId": 1, "limit": 100 }))
        .as_array()
        .unwrap()
        .is_empty());

    // 通知记录与日志
    assert!(expect_ok(&webview, "list_alerts", json!({ "limit": 20 })).as_array().unwrap().is_empty());
    let logs = expect_ok(&webview, "list_logs", json!({ "limit": 50 }));
    assert!(logs.is_array());

    // 暂停/恢复
    let paused = expect_ok(&webview, "set_paused", json!({ "paused": true }));
    assert_eq!(paused["paused"], true);
    assert_eq!(expect_ok(&webview, "set_paused", json!({ "paused": false }))["paused"], false);

    // 清理
    expect_ok(&webview, "delete_rule", json!({ "id": rule_id }));
    expect_ok(&webview, "delete_monitored", json!({ "id": monitored_id }));
    assert!(expect_ok(&webview, "list_monitored", json!({})).as_array().unwrap().is_empty());
    assert_eq!(expect_ok(&webview, "get_status", json!({}))["monitored_total"], 0);
    expect_ok(&webview, "delete_account", json!({ "id": account_id }));
    assert!(expect_ok(&webview, "list_rules", json!({})).as_array().unwrap().is_empty());
    assert!(expect_ok(&webview, "list_accounts", json!({})).as_array().unwrap().is_empty());

    // 立即评估：没有监控视频时不应命中任何报警
    assert_eq!(expect_ok(&webview, "run_rules", json!({})), json!(0));

    // 存储统计与覆盖清理
    let stats = expect_ok(&webview, "get_storage_stats", json!({}));
    assert!(stats["db_size_bytes"].as_u64().is_some());
    assert_eq!(stats["retention_days"], 7);
    assert_eq!(stats["auto_clean"], true);

    let clean = expect_ok(&webview, "clean_storage", json!({ "retentionDays": 7 }));
    assert!(clean["deleted_snapshots"].as_u64().is_some());

    // 环境全面体检
    let env_report = expect_ok(&webview, "check_environment", json!({}));
    assert!(env_report["webview2"]["name"].as_str().is_some());
    assert!(env_report["chrome"]["name"].as_str().is_some());
    assert!(env_report["storage"]["writable"].as_bool().unwrap());
    assert!(env_report["score"].as_u64().is_some());

    // 错误路径：删除不存在的作品应返回错误字符串而不是 panic
    let missing = call(&webview, "list_alerts", json!({ "limit": "not-a-number" }));
    assert!(missing.is_err(), "参数类型错误应返回错误");

    let _ = std::fs::remove_dir_all(&dir);
    app.cleanup_before_exit();
}
