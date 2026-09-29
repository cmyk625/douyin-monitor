use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// Chrome 136+ 起，`--remote-debugging-port` 只有在使用非默认 `--user-data-dir` 时才生效，
/// 因此这里始终给每个账号分配独立的浏览器数据目录。
pub struct LaunchOptions {
    pub exe: PathBuf,
    pub user_data_dir: PathBuf,
    pub headless: bool,
    pub initial_url: Option<String>,
    pub extra_args: Vec<String>,
}

pub struct ChromeProcess {
    pub child: Child,
    pub port: u16,
    pub ws_url: String,
    reaped: bool,
}

pub fn find_browser(explicit: &str) -> Option<PathBuf> {
    let explicit = explicit.trim().trim_matches('"');
    if !explicit.is_empty() {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(from_env) = std::env::var("DOUYIN_MONITOR_CHROME") {
        let path = PathBuf::from(from_env.trim().trim_matches('"'));
        if path.is_file() {
            return Some(path);
        }
    }

    candidates().into_iter().find(|path| path.is_file())
}

/// 检测已发现 Chrome 的真实版本号。
/// Windows 下 Chrome 的安装目录下有类似 "153.0.8010.55" 的子文件夹；
/// 扫描该子文件夹名称能快速、精准且无需派生子进程地获取版本号。
pub fn detect_chrome_version(exe_path: &Path) -> Option<String> {
    if let Some(parent) = exe_path.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        // 校验版本号目录（由数字和点组成，如 123.0.6312.124）
                        if name.chars().all(|c| c.is_ascii_digit() || c == '.')
                            && name.contains('.')
                            && name.len() >= 5
                        {
                            return Some(name);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn candidates() -> Vec<PathBuf> {
    let mut list = Vec::new();
    for base in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(root) = std::env::var(base) {
            list.push(PathBuf::from(&root).join("Google/Chrome/Application/chrome.exe"));
        }
    }
    list.push(PathBuf::from("C:/Program Files/Google/Chrome/Application/chrome.exe"));
    list.push(PathBuf::from("C:/Program Files (x86)/Google/Chrome/Application/chrome.exe"));
    list
}

#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn candidates() -> Vec<PathBuf> {
    ["google-chrome", "google-chrome-stable"]
        .iter()
        .filter_map(|name| which(name))
        .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn browser_version(exe: &Path) -> String {
    exe.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| exe.display().to_string())
}

/// 解析某个浏览器数据目录当前可用的调试端点。
///
/// 注意：Chrome 启动过程中可能「换壳」重启并重写 `DevToolsActivePort`（端口随之变化），
/// 因此这里每次都重新读取文件并探测 HTTP 端点，只返回真正可用的地址。
pub async fn resolve_debugger(
    profile_dir: &Path,
    client: &reqwest::Client,
    timeout: Duration,
) -> Option<(u16, String)> {
    let deadline = Instant::now() + timeout;
    let port_file = profile_dir.join("DevToolsActivePort");
    loop {
        if let Ok(text) = std::fs::read_to_string(&port_file) {
            let mut lines = text.lines();
            let port = lines.next().and_then(|line| line.trim().parse::<u16>().ok());
            let ws_path = lines.next().unwrap_or("").trim().to_string();
            if let Some(port) = port {
                if !ws_path.is_empty() && endpoint_alive(port, client).await {
                    return Some((port, format!("ws://127.0.0.1:{port}{ws_path}")));
                }
            }
        }
        if Instant::now() > deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn endpoint_alive(port: u16, client: &reqwest::Client) -> bool {
    client
        .get(format!("http://127.0.0.1:{port}/json/version"))
        .timeout(Duration::from_millis(1200))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

/// 检查该浏览器数据目录是否已有「开启了调试端口」的实例在运行。
/// 例如用户手动打开的登录窗口：采集时直接复用它，避免同一目录二次启动失败。
pub async fn live_debugger_url(profile_dir: &Path, client: &reqwest::Client) -> Option<String> {
    resolve_debugger(profile_dir, client, Duration::from_millis(2000))
        .await
        .map(|(_, ws_url)| ws_url)
}

impl ChromeProcess {
    pub fn kill(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.reaped = true;
        }
    }
}

impl Drop for ChromeProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

pub fn launch(options: &LaunchOptions) -> Result<ChromeProcess> {
    let mut command = Command::new(&options.exe);
    command
        .arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", options.user_data_dir.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-sync")
        .arg("--disable-background-networking")
        .arg("--disable-features=Translate,MediaRouter,OptimizationHints")
        .arg("--window-size=1440,960")
        .arg("--mute-audio");
    if options.headless {
        command.arg("--headless=new").arg("--disable-gpu");
    }
    for arg in &options.extra_args {
        command.arg(arg);
    }
    if let Some(url) = &options.initial_url {
        command.arg(url);
    }

    #[cfg(target_os = "windows")]
    if options.headless {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let child = command
        .spawn()
        .with_context(|| format!("启动浏览器失败：{}", options.exe.display()))?;

    let port_file = options.user_data_dir.join("DevToolsActivePort");
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        if let Ok(text) = std::fs::read_to_string(&port_file) {
            let mut lines = text.lines();
            let port = lines.next().and_then(|line| line.trim().parse::<u16>().ok());
            let ws_path = lines.next().unwrap_or("").trim().to_string();
            if let (Some(port), true) = (port, !ws_path.is_empty()) {
                return Ok(ChromeProcess {
                    child,
                    port,
                    ws_url: format!("ws://127.0.0.1:{port}{ws_path}"),
                    reaped: false,
                });
            }
        }
        if Instant::now() > deadline {
            let mut child = child;
            let _ = child.kill();
            return Err(anyhow!(
                "等待浏览器调试端口超时（{}），请确认浏览器可正常启动",
                port_file.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}
