use serde::{Deserialize, Serialize};

pub const METRIC_KEYS: [&str; 5] = ["play", "like", "comment", "share", "collect"];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Metrics {
    pub play: Option<i64>,
    pub like: Option<i64>,
    pub comment: Option<i64>,
    pub share: Option<i64>,
    pub collect: Option<i64>,
}

impl Metrics {
    pub fn is_empty(&self) -> bool {
        self.play.is_none()
            && self.like.is_none()
            && self.comment.is_none()
            && self.share.is_none()
            && self.collect.is_none()
    }

    pub fn get(&self, metric: &str) -> Option<i64> {
        match metric {
            "play" => self.play,
            "like" => self.like,
            "comment" => self.comment,
            "share" => self.share,
            "collect" => self.collect,
            _ => None,
        }
    }

    pub fn set(&mut self, metric: &str, value: Option<i64>) {
        match metric {
            "play" => self.play = value,
            "like" => self.like = value,
            "comment" => self.comment = value,
            "share" => self.share = value,
            "collect" => self.collect = value,
            _ => {}
        }
    }

    /// 用新采集到的值覆盖旧值；抓不到的字段保留上一次的数值。
    pub fn merge(&mut self, other: &Metrics) {
        for key in METRIC_KEYS {
            if let Some(value) = other.get(key) {
                self.set(key, Some(value));
            }
        }
    }

    pub fn has_growth_over(&self, other: &Metrics) -> bool {
        METRIC_KEYS.iter().any(|key| match (self.get(key), other.get(key)) {
            (Some(new), Some(old)) => new > old,
            (Some(_), None) => true,
            _ => false,
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawWork {
    #[serde(default)]
    pub aweme_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub published_at: Option<i64>,
    #[serde(default)]
    pub metrics: Metrics,
    #[serde(default)]
    pub is_approximate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub sec_uid: String,
    pub target_url: String,
    pub parse_script: String,
    pub profile_dir: String,
    pub login_state: String,
    pub enabled: bool,
    pub note: String,
    pub interval_secs: i64,
    pub created_at: i64,
    pub last_collect_at: Option<i64>,
    pub last_ok_at: Option<i64>,
    pub last_error: String,
    pub work_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInput {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub sec_uid: String,
    #[serde(default)]
    pub target_url: String,
    #[serde(default)]
    pub parse_script: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub note: String,
}

pub fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRename {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub aweme_id: String,
    pub title: String,
    #[serde(default)]
    pub author_name: String,
    pub url: String,
    pub published_at: Option<i64>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub metrics: Metrics,
    #[serde(default)]
    pub is_approximate: bool,
}

/// 监控名单中的视频：只有这些视频会被采集、评估与报警。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredWork {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub aweme_id: String,
    pub title: String,
    #[serde(default)]
    pub author_name: String,
    pub url: String,
    /// 负责人姓名
    pub owner_name: String,
    /// 负责人飞书 Open ID（用于 <at user_id="ou_xxx">）
    pub owner_open_id: String,
    pub enabled: bool,
    pub created_at: i64,
    pub last_collect_at: Option<i64>,
    pub last_error: String,
    /// 对应采集数据的 works.id（尚未采集到时为 null）
    pub work_id: Option<i64>,
    #[serde(default)]
    pub published_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredInput {
    #[serde(default)]
    pub id: Option<i64>,
    pub account_id: i64,
    /// 视频链接或作品 ID
    pub target: String,
    /// 负责人姓名（字符串，必须是代码名单里的名字）
    pub owner_name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPoint {
    pub ts: i64,
    pub play: Option<i64>,
    pub like: Option<i64>,
    pub comment: Option<i64>,
    pub share: Option<i64>,
    pub collect: Option<i64>,
    #[serde(default)]
    pub is_approximate: bool,
}

impl From<(i64, Metrics)> for SnapshotPoint {
    fn from((ts, metrics): (i64, Metrics)) -> Self {
        Self {
            ts,
            play: metrics.play,
            like: metrics.like,
            comment: metrics.comment,
            share: metrics.share,
            collect: metrics.collect,
            is_approximate: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: i64,
    pub name: String,
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub metric: String,
    pub window_minutes: i64,
    pub threshold: i64,
    pub cooldown_minutes: i64,
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleInput {
    #[serde(default)]
    pub id: Option<i64>,
    pub name: String,
    pub account_id: Option<i64>,
    pub metric: String,
    pub window_minutes: i64,
    pub threshold: i64,
    #[serde(default)]
    pub cooldown_minutes: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: i64,
    pub rule_id: i64,
    pub rule_name: String,
    pub account_id: i64,
    pub account_name: String,
    pub work_id: i64,
    pub work_title: String,
    #[serde(default)]
    pub author_name: String,
    pub work_url: String,
    pub metric: String,
    pub window_minutes: i64,
    pub baseline_value: i64,
    pub current_value: i64,
    pub delta: i64,
    pub message: String,
    pub notify_state: String,
    pub notify_detail: String,
    pub created_at: i64,
    /// 报警时的负责人名字（随报警一起保存，重发仍是当时的名字）
    #[serde(default)]
    pub owner_name: String,
    #[serde(default)]
    pub owner_open_id: String,
}

/// 某个指标在「当前」与「窗口起点」的值：消息里展示 `898(+176)` 这种增量。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricWindow {
    pub current: Option<i64>,
    pub baseline: Option<i64>,
    pub is_approximate: bool,
}

impl MetricWindow {
    pub fn delta(&self) -> Option<i64> {
        match (self.current, self.baseline) {
            (Some(current), Some(baseline)) => Some(current - baseline),
            _ => None,
        }
    }
}

/// 规则评估结果（尚未写入数据库）
#[derive(Debug, Clone)]
pub struct AlertCandidate {
    pub rule_id: i64,
    pub account_id: i64,
    pub work_id: i64,
    pub metric: String,
    pub window_minutes: i64,
    pub baseline_value: i64,
    pub current_value: i64,
    pub delta: i64,
    pub owner_name: String,
    pub owner_open_id: String,
    pub author_name: String,
    pub is_approximate: bool,
    /// 全部指标的现值/窗口基线，顺序与 METRIC_KEYS 一致（play/like/comment/share/collect）
    pub windows: [MetricWindow; 5],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: i64,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectOutcome {
    pub account_id: i64,
    pub account_name: String,
    pub ok: bool,
    pub works_found: usize,
    pub works_saved: usize,
    pub new_works: usize,
    pub snapshots: usize,
    pub login_state: String,
    pub message: String,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractPreview {
    pub ok: bool,
    pub blocked: bool,
    pub reason: String,
    pub page_url: String,
    pub page_title: String,
    pub works: usize,
    pub sample: Vec<RawWork>,
    pub debug: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureEntry {
    pub url: String,
    pub status: i64,
    pub bytes: usize,
    pub mime: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    pub ok: bool,
    pub saved: String,
    pub count: usize,
    pub entries: Vec<CaptureEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyResult {
    pub ok: bool,
    pub code: i64,
    pub msg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCheck {
    pub login_state: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub collecting: bool,
    pub paused: bool,
    pub current_account: String,
    pub last_run_at: Option<i64>,
    pub next_run_at: Option<i64>,
    pub accounts_total: i64,
    pub accounts_ok: i64,
    pub works_total: i64,
    pub snapshots_total: i64,
    pub monitored_total: i64,
    pub alerts_today: i64,
    pub feishu_configured: bool,
    pub feishu_enabled: bool,
    pub chrome_path: String,
    pub data_dir: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub db_size_bytes: u64,
    pub snapshots_count: i64,
    pub works_count: i64,
    pub monitored_count: i64,
    pub alerts_count: i64,
    pub oldest_snapshot_ts: Option<i64>,
    pub retention_days: i64,
    pub auto_clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanStorageResult {
    pub deleted_snapshots: usize,
    pub deleted_works: usize,
    pub db_size_before: u64,
    pub db_size_after: u64,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentReport {
    pub webview2: ComponentStatus,
    pub chrome: ComponentStatus,
    pub network: NetworkStatus,
    pub storage: StorageHealthStatus,
    pub os: OsInfo,
    pub all_passed: bool,
    pub score: u8,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub ok: bool,
    pub name: String,
    pub version: Option<String>,
    pub path: Option<String>,
    pub detail: String,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub ok: bool,
    pub douyin_main_ok: bool,
    pub douyin_main_latency_ms: Option<u64>,
    pub douyin_short_ok: bool,
    pub douyin_short_latency_ms: Option<u64>,
    pub feishu_ok: Option<bool>,
    pub feishu_latency_ms: Option<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageHealthStatus {
    pub ok: bool,
    pub writable: bool,
    pub data_dir: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub db_size_bytes: u64,
    pub db_ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub os_name: String,
    pub arch: String,
}
