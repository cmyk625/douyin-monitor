use crate::logging::Logger;
use crate::models::RuntimeStatus;
use crate::settings::Settings;
use anyhow::{anyhow, Result};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Default)]
pub struct RuntimeState {
    pub paused: bool,
    pub collecting: bool,
    pub current_account: String,
    pub last_run_at: Option<i64>,
    pub next_run_at: Option<i64>,
    pub last_error: String,
}

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub http: reqwest::Client,
    pub logger: Logger,
    pub runtime: Mutex<RuntimeState>,
    pub data_dir: PathBuf,
    pub version: String,
    pub app: Mutex<Option<AppHandle>>,
    pub browser_path: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new(db: Connection, data_dir: PathBuf, version: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("douyin-monitor/0.1 (+tauri)")
            .build()
            .unwrap_or_default();
        Self {
            db: Arc::new(Mutex::new(db)),
            http,
            logger: Logger::new(),
            runtime: Mutex::new(RuntimeState::default()),
            data_dir,
            version,
            app: Mutex::new(None),
            browser_path: Mutex::new(None),
        }
    }

    pub fn attach(&self, app: AppHandle) {
        self.logger.attach(app.clone());
        if let Ok(mut slot) = self.app.lock() {
            *slot = Some(app);
        }
    }

    pub fn emit<T: serde::Serialize + Clone>(&self, event: &str, payload: T) {
        if let Some(app) = self.app_handle() {
            let _ = app.emit(event, payload);
        }
    }

    /// 当前 AppHandle（应用尚未就绪时为 None）。
    pub fn app_handle(&self) -> Option<AppHandle> {
        self.app.lock().ok().and_then(|app| app.clone())
    }

    /// 数据库访问：闭包内不要 await。
    pub fn db<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.db.lock().map_err(|_| anyhow!("数据库锁失效"))?;
        f(&guard)
    }

    pub fn settings(&self) -> Settings {
        self.db(|conn| Ok(Settings::load(conn))).unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.db(|conn| settings.save(conn))
    }

    pub fn profile_dir(&self, account_id: i64, name: &str) -> PathBuf {
        self.data_dir
            .join("chrome-profiles")
            .join(format!("acc-{account_id}-{}", crate::util::sanitize_component(name)))
    }

    pub fn browser(&self, settings: &Settings) -> Result<PathBuf> {
        if let Ok(cache) = self.browser_path.lock() {
            if let Some(path) = cache.as_ref() {
                return Ok(path.clone());
            }
        }
        let found = crate::cdp::find_browser(&settings.chrome_path)
            .ok_or_else(|| anyhow!("未检测到 Google Chrome 浏览器，请前往 https://www.google.cn/chrome/ 下载安装，或在设置中指定 chrome.exe 路径"))?;
        if let Ok(mut cache) = self.browser_path.lock() {
            *cache = Some(found.clone());
        }
        Ok(found)
    }

    pub fn status(&self) -> RuntimeStatus {
        let settings = self.settings();
        let (works_total, snapshots_total) = self
            .db(crate::repo_work::totals)
            .unwrap_or((0, 0));
        let accounts = self.db(crate::repo::list_accounts).unwrap_or_default();
        let monitored_total = self
            .db(crate::repo_monitored::count_enabled)
            .unwrap_or(0);
        let alerts_today = self
            .db(|conn| crate::repo_alert::count_alerts_since(conn, crate::util::today_start_ts()))
            .unwrap_or(0);
        let runtime = self.runtime.lock().map(|guard| RuntimeState {
            paused: guard.paused,
            collecting: guard.collecting,
            current_account: guard.current_account.clone(),
            last_run_at: guard.last_run_at,
            next_run_at: guard.next_run_at,
            last_error: guard.last_error.clone(),
        });
        let runtime = runtime.unwrap_or_default();
        let chrome_path = self
            .browser_path
            .lock()
            .ok()
            .and_then(|path| path.clone())
            .map(|path| path.display().to_string())
            .or_else(|| {
                crate::cdp::find_browser(&settings.chrome_path).map(|path| path.display().to_string())
            })
            .unwrap_or_default();

        let feishu_cfg = crate::config::AppConfig::load().feishu;
        let feishu_configured = crate::feishu::is_configured(&feishu_cfg.webhook);
        let feishu_enabled = feishu_cfg.enabled;

        RuntimeStatus {
            collecting: runtime.collecting,
            paused: runtime.paused,
            current_account: runtime.current_account,
            last_run_at: runtime.last_run_at,
            next_run_at: runtime.next_run_at,
            accounts_total: accounts.len() as i64,
            accounts_ok: accounts.iter().filter(|acc| acc.login_state == "ok").count() as i64,
            works_total,
            snapshots_total,
            monitored_total,
            alerts_today,
            feishu_configured,
            feishu_enabled,
            chrome_path,
            data_dir: self.data_dir.display().to_string(),
            version: self.version.clone(),
        }
    }

    pub fn emit_status(&self) {
        let status = self.status();
        self.emit("status", status);
    }

    /// 打开一个可见浏览器窗口，供用户扫码登录（进程不托管，用户关闭窗口即可）。
    pub fn open_browser_window(&self, profile_dir: &PathBuf, url: &str) -> Result<()> {
        let settings = self.settings();
        let exe = self.browser(&settings)?;
        crate::util::ensure_dir(profile_dir)?;

        // 必须使用 `--user-data-dir=<目录>` 的单参数写法。拆成两个参数时 Chrome 不会把目录
        // 当作开关的值，而是把它当成「要打开的本地路径」交给系统外壳处理，于是弹出
        // 资源管理器窗口（表现为打开了一个 C 盘目录）。
        let mut user_data_dir = std::ffi::OsString::from("--user-data-dir=");
        user_data_dir.push(profile_dir.as_os_str());

        let mut child = std::process::Command::new(&exe)
            .arg("--remote-debugging-port=0")
            .arg(&user_data_dir)
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--new-window")
            .arg("--window-size=1280,900")
            .arg(url)
            .spawn()
            .map_err(|err| anyhow!("启动浏览器失败（{}）：{err}", exe.display()))?;

        // 该目录已有实例在运行时，Chrome 会把新窗口转交给既有实例并立即退出（退出码 0）；
        // 只有非 0 退出码才说明启动失败（例如目录不可写、参数非法）。
        std::thread::sleep(std::time::Duration::from_millis(600));
        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() {
                return Err(anyhow!(
                    "浏览器启动失败（退出码 {status}），请检查账号目录是否可写：{}",
                    profile_dir.display()
                ));
            }
        }
        Ok(())
    }

    pub fn data_subdir(&self, name: &str) -> Result<PathBuf> {
        let path = self.data_dir.join(name);
        crate::util::ensure_dir(&path)?;
        Ok(path)
    }

    pub fn open_path(&self, path: &std::path::Path) -> Result<()> {
        let target = path.display().to_string();
        opener::open(&target).map_err(|err| anyhow!("打开 {target} 失败：{err}"))
    }
}

/// 快捷：把账号资料目录解析为绝对路径。
pub fn account_profile_dir(state: &AppState, account: &crate::models::Account) -> PathBuf {
    if account.profile_dir.trim().is_empty() {
        state.profile_dir(account.id, &account.name)
    } else {
        PathBuf::from(&account.profile_dir)
    }
}

/// 由数据目录构造 AppState（供 setup 使用）。
pub fn build_state(data_dir: PathBuf, version: String) -> Result<AppState> {
    crate::util::ensure_dir(&data_dir)?;
    let db_path = data_dir.join("douyin-monitor.db");
    let conn = crate::db::open(&db_path)?;
    Ok(AppState::new(conn, data_dir, version))
}

/// 让 AppHandle 在日志与事件中可用。
pub fn attach_app(state: &AppState, app: &AppHandle) {
    state.attach(app.clone());
    let _ = app.emit("status", state.status());
}
