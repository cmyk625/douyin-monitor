use crate::models::LogEntry;
use crate::util::now_ts;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

const CAPACITY: usize = 600;

/// 对日志内容进行安全脱敏与规范化，屏蔽底层协议名称（如 CDP）等技术细节
pub fn sanitize_log_message(msg: &str) -> String {
    let replaced = msg
        .replace("CDP Fetch 域", "网络拦截")
        .replace("CDP Fetch.getResponseBody", "网络拦截读取响应体")
        .replace("CDP Fetch", "网络拦截")
        .replace("CDP Network 域", "网络监听")
        .replace("CDP Network.getResponseBody", "网络响应体")
        .replace("CDP Network", "网络监听")
        .replace("CDP 响应体", "网络响应体")
        .replace("CDP 网络响应", "网络响应")
        .replace("CDP 会话", "浏览器会话")
        .replace("CDP 端点", "浏览器端点")
        .replace("CDP 连接", "浏览器连接")
        .replace("CDP 状态", "浏览器状态")
        .replace("CDP 调试", "浏览器调试")
        .replace("CDP", "浏览器")
        .replace("cdp", "browser");

    // 清理因中英混排空格替换后遗留的断词空格
    replaced
        .replace("连接 浏览器", "连接浏览器")
        .replace("已连接 浏览器", "已连接浏览器")
        .replace("浏览器 服务", "浏览器服务")
        .replace(" 浏览器 ", "浏览器")
}

#[derive(Clone, Default)]
pub struct Logger {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    buffer: Mutex<VecDeque<LogEntry>>,
    app: Mutex<Option<AppHandle>>,
}

impl Logger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&self, app: AppHandle) {
        if let Ok(mut slot) = self.inner.app.lock() {
            *slot = Some(app);
        }
    }

    pub fn log(&self, level: &str, target: &str, message: impl Into<String>) {
        let raw_target = target.trim();
        // 隐去 CDP 描述：目标分类为 cdp 时统一转为 browser
        let sanitized_target = if raw_target.eq_ignore_ascii_case("cdp") {
            "browser"
        } else {
            raw_target
        };

        let raw_message = message.into();
        let sanitized_message = sanitize_log_message(&raw_message);

        let entry = LogEntry {
            ts: now_ts(),
            level: level.to_string(),
            target: sanitized_target.to_string(),
            message: sanitized_message,
        };
        eprintln!("[{}] {} {} {}", entry.level, entry.ts, entry.target, entry.message);
        if let Ok(mut buffer) = self.inner.buffer.lock() {
            buffer.push_back(entry.clone());
            while buffer.len() > CAPACITY {
                buffer.pop_front();
            }
        }
        if let Some(app) = self.inner.app.lock().ok().and_then(|app| app.clone()) {
            let _ = app.emit("log", entry);
        }
    }

    pub fn info(&self, target: &str, message: impl Into<String>) {
        self.log("INFO", target, message);
    }

    pub fn warn(&self, target: &str, message: impl Into<String>) {
        self.log("WARN", target, message);
    }

    pub fn error(&self, target: &str, message: impl Into<String>) {
        self.log("ERROR", target, message);
    }

    /// 最近日志，最新的在前。
    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        let buffer = match self.inner.buffer.lock() {
            Ok(buffer) => buffer,
            Err(_) => return Vec::new(),
        };
        buffer.iter().rev().take(limit).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_log_message() {
        assert_eq!(
            sanitize_log_message("CDP Fetch 域截获视频接口成功"),
            "网络拦截截获视频接口成功"
        );
        assert_eq!(
            sanitize_log_message("连接 CDP 端点超时"),
            "连接浏览器端点超时"
        );
        assert_eq!(
            sanitize_log_message("CDP 连接已关闭"),
            "浏览器连接已关闭"
        );
        assert_eq!(
            sanitize_log_message("普通日志消息无需变动"),
            "普通日志消息无需变动"
        );
    }

    #[test]
    fn test_logger_sanitizes_target_and_message() {
        let logger = Logger::new();
        logger.info("cdp", "已连接 CDP 服务");
        let logs = logger.recent(10);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].target, "browser");
        assert_eq!(logs[0].message, "已连接浏览器服务");
    }
}
