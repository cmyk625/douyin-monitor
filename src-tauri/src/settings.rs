use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_wait() -> i64 {
    8
}

fn default_max_works() -> i64 {
    50
}

fn default_scroll_rounds() -> i64 {
    6
}

fn default_retention_days() -> i64 {
    7
}

/// 设置（整体以 JSON 存在 settings 表中）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_true")]
    pub headless: bool,
    #[serde(default = "default_wait")]
    pub page_wait_secs: i64,
    #[serde(default = "default_max_works")]
    pub max_works: i64,
    #[serde(default = "default_scroll_rounds")]
    pub scroll_rounds: i64,
    /// 自定义作品卡片选择器，每行一个；为空时使用内置选择器。
    #[serde(default)]
    pub card_selectors: String,
    /// 指标标签覆盖，JSON 形如 {"play": ["播放量", "播放"]}。
    #[serde(default)]
    pub metric_labels: String,
    /// 浏览器可执行文件路径覆盖；为空时自动探测。
    #[serde(default)]
    pub chrome_path: String,
    /// 开机自启动：None = 未配置（首次运行默认开启）；Some(true/false) = 用户的选择。
    #[serde(default)]
    pub autostart: Option<bool>,
    /// 快照数据保留天数（默认 7 天）
    #[serde(default = "default_retention_days")]
    pub retention_days: i64,
    /// 是否在采集完成后自动滚动覆盖清理超期快照（默认 true）
    #[serde(default = "default_true")]
    pub auto_clean: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            headless: true,
            page_wait_secs: default_wait(),
            max_works: default_max_works(),
            scroll_rounds: default_scroll_rounds(),
            card_selectors: String::new(),
            metric_labels: String::new(),
            chrome_path: String::new(),
            autostart: None,
            retention_days: default_retention_days(),
            auto_clean: true,
        }
    }
}

impl Settings {
    pub fn clamp(&mut self) {
        self.page_wait_secs = self.page_wait_secs.clamp(2, 60);
        self.max_works = self.max_works.clamp(5, 500);
        self.scroll_rounds = self.scroll_rounds.clamp(0, 50);
        self.chrome_path = self.chrome_path.trim().trim_matches('"').to_string();
        self.retention_days = self.retention_days.clamp(1, 365);
    }

    pub fn load(conn: &Connection) -> Self {
        let raw: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = 'app'", [], |row| row.get(0))
            .ok();
        match raw {
            Some(text) => serde_json::from_str(&text).unwrap_or_default(),
            None => Settings::default(),
        }
    }

    pub fn save(&self, conn: &Connection) -> Result<()> {
        let text = serde_json::to_string(self)?;
        conn.execute(
            "INSERT INTO settings(key, value) VALUES('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![text],
        )?;
        Ok(())
    }
}
