//! 开机自启动：默认开启，可在「设置 → 启动与托盘」里关闭。
//!
//! Windows 下写入注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`（项名见 [`APP_NAME`]），
//! macOS / Linux 由 tauri-plugin-autostart 处理。

use crate::state::AppState;
use anyhow::{anyhow, Result};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// 由系统拉起时附加的参数：带上它说明是开机自启动，启动后只留托盘。
pub const ARG: &str = "--autostart";
/// 启动项名称（Windows 注册表里的项名）。
pub const APP_NAME: &str = "DouyinMonitor";

/// 判断参数里是否包含自启动标记（独立成函数便于测试）。
pub fn requested_by_args<I: IntoIterator<Item = String>>(args: I) -> bool {
    args.into_iter().any(|arg| arg == ARG)
}

/// 本次进程是否由开机自启动拉起。
pub fn launched_by_system() -> bool {
    requested_by_args(std::env::args())
}

pub fn enable(app: &AppHandle) -> Result<()> {
    app.autolaunch()
        .enable()
        .map_err(|err| anyhow!("开启开机自启动失败：{err}"))
}

pub fn disable(app: &AppHandle) -> Result<()> {
    app.autolaunch()
        .disable()
        .map_err(|err| anyhow!("关闭开机自启动失败：{err}"))
}

pub fn is_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// 启动时同步自启动状态：
/// - `Some(true)`：重新写入一次，保证注册表指向当前 exe（升级/换目录后仍有效）；
/// - `Some(false)`：尊重用户的选择，不动；
/// - `None`（首次运行）：默认开启并记录到设置里，之后由用户决定。
pub fn sync_on_startup(state: &AppState, app: &AppHandle) {
    let mut settings = state.settings();
    match settings.autostart {
        Some(true) => {
            if let Err(err) = enable(app) {
                state.logger.warn("autostart", format!("刷新开机自启动失败：{err:#}"));
            }
        }
        Some(false) => {}
        None => match enable(app) {
            Ok(()) => {
                settings.autostart = Some(true);
                if let Err(err) = state.save_settings(&settings) {
                    state.logger.warn("autostart", format!("保存自启动设置失败：{err:#}"));
                }
                state
                    .logger
                    .info("autostart", "已默认开启开机自启动（可在「设置 → 启动与托盘」关闭）");
            }
            Err(err) => state.logger.warn("autostart", format!("开启开机自启动失败：{err:#}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn detects_autostart_arg() {
        assert!(requested_by_args(args(&["app.exe", ARG])));
        assert!(!requested_by_args(args(&["app.exe", "--hidden"])));
        assert!(!requested_by_args(args(&[])));
    }
}
