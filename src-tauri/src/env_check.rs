use crate::models::{
    ComponentStatus, EnvironmentReport, NetworkStatus, OsInfo, StorageHealthStatus,
};
use crate::state::AppState;
use std::path::Path;
use std::time::{Duration, Instant};

const WEBVIEW2_DOWNLOAD_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";
const CHROME_DOWNLOAD_URL: &str = "https://www.google.cn/chrome/";

/// 检测 Microsoft Edge WebView2 运行时环境
pub fn detect_webview2() -> ComponentStatus {
    #[cfg(target_os = "windows")]
    {
        // 1. 优先调用 Tauri 2 原生 webview_version()
        if let Ok(ver) = tauri::webview_version() {
            if !ver.trim().is_empty() {
                return ComponentStatus {
                    ok: true,
                    name: "Microsoft Edge WebView2".to_string(),
                    version: Some(ver.trim().to_string()),
                    path: None,
                    detail: format!("WebView2 运行时就绪 (v{})，客户端界面交互引擎正常", ver.trim()),
                    download_url: Some(WEBVIEW2_DOWNLOAD_URL.to_string()),
                };
            }
        }

        // 2. 备用目录扫描（EdgeWebView 常见安装路径）
        for base in [
            r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application",
            r"C:\Program Files\Microsoft\EdgeWebView\Application",
        ] {
            let p = Path::new(base);
            if p.is_dir() {
                if let Some(ver) = crate::cdp::detect_chrome_version(&p.join("msedge.exe")) {
                    return ComponentStatus {
                        ok: true,
                        name: "Microsoft Edge WebView2".to_string(),
                        version: Some(ver.clone()),
                        path: Some(base.to_string()),
                        detail: format!("WebView2 运行时就绪 (v{ver})，客户端界面交互引擎正常"),
                        download_url: Some(WEBVIEW2_DOWNLOAD_URL.to_string()),
                    };
                }
            }
        }

        ComponentStatus {
            ok: false,
            name: "Microsoft Edge WebView2".to_string(),
            version: None,
            path: None,
            detail: "未检测到 Microsoft Edge WebView2 运行时，应用可能无法正常显示窗口".to_string(),
            download_url: Some(WEBVIEW2_DOWNLOAD_URL.to_string()),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        ComponentStatus {
            ok: true,
            name: "System WebKit".to_string(),
            version: Some("Native".to_string()),
            path: None,
            detail: "系统原生 WebKit 运行正常".to_string(),
            download_url: None,
        }
    }
}

/// 检测 Google Chrome 浏览器环境（数据采集与自动化引擎核心）
pub fn detect_chrome(state: &AppState) -> ComponentStatus {
    let settings = state.settings();
    match state.browser(&settings) {
        Ok(path) => {
            let version = crate::cdp::detect_chrome_version(&path);
            let ver_text = version
                .as_ref()
                .map(|v| format!("v{v}"))
                .unwrap_or_else(|| "版本已适配".to_string());
            ComponentStatus {
                ok: true,
                name: "Google Chrome".to_string(),
                version,
                path: Some(path.display().to_string()),
                detail: format!("Google Chrome {ver_text} 已就绪，后台自动化采集服务正常"),
                download_url: Some(CHROME_DOWNLOAD_URL.to_string()),
            }
        }
        Err(_) => ComponentStatus {
            ok: false,
            name: "Google Chrome".to_string(),
            version: None,
            path: None,
            detail: "未检测到 Google Chrome 浏览器，后台自动化采集无法工作。请下载安装 Chrome 或在设置中指定路径。".to_string(),
            download_url: Some(CHROME_DOWNLOAD_URL.to_string()),
        },
    }
}

/// 检测网络与核心接口连通性（抖音官方主站、短链解析域名、飞书开放平台）
pub async fn check_network(client: &reqwest::Client, check_feishu: bool) -> NetworkStatus {
    // 1. 抖音主站检测
    let start_douyin = Instant::now();
    let douyin_resp = client
        .get("https://www.douyin.com")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .timeout(Duration::from_millis(4000))
        .send()
        .await;

    let (douyin_ok, douyin_latency) = match douyin_resp {
        Ok(resp) if resp.status().is_success() || resp.status().is_redirection() => {
            (true, Some(start_douyin.elapsed().as_millis() as u64))
        }
        _ => (false, None),
    };

    // 2. 抖音短链域名检测
    let start_short = Instant::now();
    let short_resp = client
        .head("https://v.douyin.com")
        .timeout(Duration::from_millis(4000))
        .send()
        .await;

    let (short_ok, short_latency) = match short_resp {
        Ok(_) => (true, Some(start_short.elapsed().as_millis() as u64)),
        _ => (false, None),
    };

    // 3. 飞书开放平台连通性检测（若配置了飞书通知）
    let (feishu_ok, feishu_latency) = if check_feishu {
        let start_feishu = Instant::now();
        let feishu_resp = client
            .head("https://open.feishu.cn")
            .timeout(Duration::from_millis(4000))
            .send()
            .await;
        match feishu_resp {
            Ok(_) => (Some(true), Some(start_feishu.elapsed().as_millis() as u64)),
            _ => (Some(false), None),
        }
    } else {
        (None, None)
    };

    let ok = douyin_ok && short_ok && feishu_ok.unwrap_or(true);
    let detail = if !douyin_ok {
        "无法连通抖音主站 (www.douyin.com)，请检查网络连接、系统代理或 DNS 设置".to_string()
    } else if !short_ok {
        "抖音短链服务 (v.douyin.com) 连通异常，App 分享链接转换可能受影响".to_string()
    } else if feishu_ok == Some(false) {
        "无法连接飞书开放平台 (open.feishu.cn)，报警消息可能无法及时推送".to_string()
    } else {
        format!(
            "网络连通良好（抖音主站延迟 {}ms{}）",
            douyin_latency.unwrap_or(0),
            feishu_latency
                .map(|ms| format!("，飞书延迟 {ms}ms"))
                .unwrap_or_default()
        )
    };

    NetworkStatus {
        ok,
        douyin_main_ok: douyin_ok,
        douyin_main_latency_ms: douyin_latency,
        douyin_short_ok: short_ok,
        douyin_short_latency_ms: short_latency,
        feishu_ok,
        feishu_latency_ms: feishu_latency,
        detail,
    }
}

/// 检测磁盘可用空间与 AppData 数据目录读写权限
pub fn check_storage(data_dir: &Path, state: &AppState) -> StorageHealthStatus {
    // 1. 读写权限测试
    let writable = test_writable(data_dir);

    // 2. 磁盘剩余空间
    let (free_bytes, total_bytes) = get_disk_free_space(data_dir).unwrap_or((0, 0));

    // 3. SQLite 数据库健康检测
    let db_path = data_dir.join("douyin-monitor.db");
    let db_size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let db_ok = state
        .db(|conn| {
            let res: String = conn.query_row("PRAGMA quick_check;", [], |r| r.get(0))?;
            Ok(res == "ok")
        })
        .unwrap_or(false);

    // 4. 判定与描述（可用空间低于 500MB 时触发警告）
    let is_disk_low = free_bytes > 0 && free_bytes < 500 * 1024 * 1024;
    let ok = writable && db_ok && !is_disk_low;

    let free_gb = free_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let detail = if !writable {
        format!("本地数据目录缺少写入权限：{}", data_dir.display())
    } else if !db_ok {
        "SQLite 数据库完整性检查异常，建议在系统设置中执行数据备份或覆盖清理".to_string()
    } else if is_disk_low {
        format!("数据目录所在磁盘剩余空间不足 500MB (剩余 {free_gb:.1} GB)，快照可能写入失败")
    } else {
        format!("存储空间充足 (可用 {free_gb:.1} GB)，SQLite 数据完整性正常")
    };

    StorageHealthStatus {
        ok,
        writable,
        data_dir: data_dir.display().to_string(),
        free_bytes,
        total_bytes,
        db_size_bytes,
        db_ok,
        detail,
    }
}

fn test_writable(dir: &Path) -> bool {
    if !dir.exists() {
        if std::fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    let test_file = dir.join(".env_health_check.tmp");
    let test_content = b"health_check";
    if std::fs::write(&test_file, test_content).is_err() {
        return false;
    }
    let read_back = std::fs::read(&test_file).unwrap_or_default();
    let _ = std::fs::remove_file(&test_file);
    read_back == test_content
}

#[cfg(target_os = "windows")]
fn get_disk_free_space(path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free_available: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free: u64 = 0;
    unsafe {
        let res = GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_available,
            &mut total_bytes,
            &mut total_free,
        );
        if res != 0 {
            Some((free_available, total_bytes))
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_disk_free_space(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// 获取操作系统基本信息
pub fn get_os_info() -> OsInfo {
    OsInfo {
        os_name: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

/// 执行全量环境健康体检，输出综合评分与报告
pub async fn run_full_check(state: &AppState) -> EnvironmentReport {
    let feishu_cfg = crate::config::AppConfig::load().feishu;
    let has_feishu = crate::feishu::is_configured(&feishu_cfg.webhook);

    let webview2 = detect_webview2();
    let chrome = detect_chrome(state);
    let network = check_network(&state.http, has_feishu).await;
    let storage = check_storage(&state.data_dir, state);
    let os = get_os_info();

    let mut score = 100u8;
    if !webview2.ok {
        score = score.saturating_sub(40);
    }
    if !chrome.ok {
        score = score.saturating_sub(40);
    }
    if !storage.ok {
        score = score.saturating_sub(30);
    }
    if !network.ok {
        score = score.saturating_sub(20);
    }

    let all_passed = webview2.ok && chrome.ok && network.ok && storage.ok;
    let summary = if all_passed {
        "核心运行依赖与网络均已就绪，软件可完美稳定运行。".to_string()
    } else {
        let mut issues = Vec::new();
        if !webview2.ok {
            issues.push("缺少 WebView2 运行时");
        }
        if !chrome.ok {
            issues.push("未检测到 Google Chrome");
        }
        if !network.ok {
            issues.push("网络连通受限");
        }
        if !storage.ok {
            issues.push("存储或数据库异常");
        }
        format!("检测到运行环境存在隐患：{}，请参考下方诊断进行处理", issues.join("、"))
    };

    EnvironmentReport {
        webview2,
        chrome,
        network,
        storage,
        os,
        all_passed,
        score,
        summary,
    }
}
