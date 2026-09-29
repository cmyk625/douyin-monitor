pub mod autostart;
pub mod cdp;
pub mod cdp_client;
pub mod cdp_page;
pub mod collect;
pub mod collector;
pub mod commands;
pub mod config;
pub mod db;
pub mod env_check;
pub mod extractor;
pub mod extractor_js;
pub mod feishu;
#[cfg(test)]
mod ipc_test;
pub mod logging;
pub mod login_watch;
pub mod models;
pub mod repo;
pub mod repo_alert;
pub mod repo_monitored;
pub mod repo_work;
pub mod rules;
pub mod scheduler;
pub mod settings;
pub mod state;
pub mod util;

use std::sync::Arc;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(
            // 开机自启动：系统拉起时带上 --autostart，启动后只留托盘
            tauri_plugin_autostart::Builder::new()
                .app_name(autostart::APP_NAME)
                .arg(autostart::ARG)
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir()?;
            let version = app.package_info().version.to_string();
            let state = Arc::new(crate::state::build_state(data_dir, version)?);
            app.manage(state.clone());
            state::attach_app(&state, &handle);
            setup_tray(app)?;
            state
                .logger
                .info("app", format!("数据目录：{}", state.data_dir.display()));

            // 初始化或读取 config.toml（若数据库曾保存过飞书配置，则首次自动迁移过去）
            let old_feishu = state.db(|conn| {
                let raw: Option<String> = conn
                    .query_row("SELECT value FROM settings WHERE key = 'app'", [], |row| row.get(0))
                    .ok();
                if let Some(text) = raw {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                        let webhook = v.get("feishu_webhook").and_then(|s| s.as_str()).unwrap_or("").trim().to_string();
                        let secret = v.get("feishu_secret").and_then(|s| s.as_str()).unwrap_or("").trim().to_string();
                        let keyword = v.get("feishu_keyword").and_then(|s| s.as_str()).unwrap_or("").trim().to_string();
                        let enabled = v.get("notify_enabled").and_then(|b| b.as_bool()).unwrap_or(true);
                        if !webhook.is_empty() {
                            return Ok(Some(crate::config::FeishuConfig {
                                enabled,
                                webhook,
                                secret,
                                keyword,
                            }));
                        }
                    }
                }
                Ok(None)
            }).unwrap_or(None);

            crate::config::AppConfig::init_base_dir(&state.data_dir);
            let _ = crate::config::AppConfig::load_or_init(old_feishu);
            state
                .logger
                .info("config", format!("配置文件路径：{}", crate::config::AppConfig::config_path().display()));

            autostart::sync_on_startup(&state, &handle);
            if autostart::launched_by_system() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
                state
                    .logger
                    .info("autostart", "本次由开机自启动拉起，主窗口已隐藏（托盘常驻）");
            }
            scheduler::spawn(state.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                if let Some(state) = window.try_state::<Arc<state::AppState>>() {
                    state
                        .logger
                        .info("app", "主窗口已隐藏，程序继续在系统托盘运行（托盘菜单可退出）");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_accounts,
            commands::create_account,
            commands::update_account,
            commands::delete_account,
            commands::open_login_window,
            commands::check_login,
            commands::collect_now,
            commands::collect_all,
            commands::recheck_chrome,
            commands::preview_extract,
            commands::list_monitored,
            commands::save_monitored,
            commands::delete_monitored,
            commands::list_works,
            commands::work_trend,
            commands::delete_work,
            commands::list_rules,
            commands::save_rule,
            commands::delete_rule,
            commands::run_rules,
            commands::list_alerts,
            commands::clear_alerts,
            commands::resend_alert,
            commands::get_settings,
            commands::save_settings,
            commands::set_autostart,
            commands::get_feishu_config,
            commands::save_feishu_config,
            commands::test_feishu,
            commands::get_status,
            commands::set_paused,
            commands::list_logs,
            commands::open_url,
            commands::open_data_dir,
            commands::open_config_file,
            commands::get_storage_stats,
            commands::clean_storage,
            commands::check_environment,
        ])
        .run(tauri::generate_context!())
        .expect("抖音作品监控启动失败");
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", "显示主界面").build(app)?;
    let pause = MenuItemBuilder::with_id("pause", "暂停 / 恢复自动采集").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&show, &pause])
        .separator()
        .item(&quit)
        .build()?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("抖音作品数据监控")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "pause" => {
                if let Some(state) = app.try_state::<Arc<state::AppState>>() {
                    let paused = {
                        let mut runtime = state.runtime.lock().expect("状态锁失效");
                        runtime.paused = !runtime.paused;
                        runtime.paused
                    };
                    state
                        .logger
                        .info("scheduler", if paused { "已暂停自动采集" } else { "已恢复自动采集" });
                    state.emit_status();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}
