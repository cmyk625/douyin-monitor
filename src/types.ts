export interface Metrics {
  play: number | null;
  like: number | null;
  comment: number | null;
  share: number | null;
  collect: number | null;
}

export interface Account {
  id: number;
  name: string;
  sec_uid: string;
  target_url: string;
  parse_script: string;
  profile_dir: string;
  login_state: string;
  enabled: boolean;
  note: string;
  interval_secs: number;
  created_at: number;
  last_collect_at: number | null;
  last_ok_at: number | null;
  last_error: string;
  work_count: number;
}

export interface AccountInput {
  id: number | null;
  name: string;
  sec_uid: string;
  target_url: string;
  parse_script: string;
  enabled: boolean;
  note: string;
}

export interface Work {
  id: number;
  account_id: number;
  account_name: string;
  author_name: string;
  aweme_id: string;
  title: string;
  url: string;
  published_at: number | null;
  first_seen_at: number;
  last_seen_at: number;
  metrics: Metrics;
  is_approximate: boolean;
}

export interface SnapshotPoint {
  ts: number;
  play: number | null;
  like: number | null;
  comment: number | null;
  share: number | null;
  collect: number | null;
  is_approximate?: boolean;
}

/** 监控名单中的视频：只有这些视频会被采集、评估与报警。 */
export interface MonitoredWork {
  id: number;
  account_id: number;
  account_name: string;
  author_name: string;
  aweme_id: string;
  title: string;
  url: string;
  /** 负责人名字（报警消息里替换用） */
  owner_name: string;
  enabled: boolean;
  created_at: number;
  last_collect_at: number | null;
  last_error: string;
  /** 对应采集数据（works.id），尚未采集到为 null */
  work_id: number | null;
  /** 作品发布时间戳（秒），尚未采集到为 null */
  published_at: number | null;
  last_seen_at: number | null;
  metrics: Metrics;
}

export interface MonitoredInput {
  id: number | null;
  account_id: number;
  /** 视频链接或作品 ID */
  target: string;
  /** 负责人名字（手填，会替换进报警内容） */
  owner_name: string;
  enabled: boolean;
}

export interface Rule {
  id: number;
  name: string;
  account_id: number | null;
  account_name: string | null;
  metric: string;
  window_minutes: number;
  threshold: number;
  cooldown_minutes?: number;
  enabled: boolean;
  created_at: number;
}

export interface RuleInput {
  id: number | null;
  name: string;
  account_id: number | null;
  metric: string;
  window_minutes: number;
  threshold: number;
  cooldown_minutes?: number;
  enabled: boolean;
}

export interface Alert {
  id: number;
  rule_id: number;
  rule_name: string;
  account_id: number;
  account_name: string;
  author_name: string;
  work_id: number;
  work_title: string;
  work_url: string;
  metric: string;
  window_minutes: number;
  baseline_value: number;
  current_value: number;
  delta: number;
  message: string;
  notify_state: string;
  notify_detail: string;
  created_at: number;
  /** 报警时的负责人名字 */
  owner_name: string;
}

export interface FeishuConfig {
  enabled: boolean;
  webhook: string;
  secret: string;
  keyword: string;
}

export interface Settings {
  headless: boolean;
  page_wait_secs: number;
  max_works: number;
  scroll_rounds: number;
  card_selectors: string;
  metric_labels: string;
  chrome_path: string;
  /** 开机自启动：null = 未配置（首次运行默认开启） */
  autostart: boolean | null;
  /** 快照数据保留天数（默认 7 天） */
  retention_days: number;
  /** 是否在采集完成后自动滚动覆盖清理超期快照（默认 true） */
  auto_clean: boolean;
}

export interface StorageStats {
  db_size_bytes: number;
  snapshots_count: number;
  works_count: number;
  monitored_count: number;
  alerts_count: number;
  oldest_snapshot_ts: number | null;
  retention_days: number;
  auto_clean: boolean;
}

export interface CleanStorageResult {
  deleted_snapshots: number;
  deleted_works: number;
  db_size_before: number;
  db_size_after: number;
  freed_bytes: number;
}

export interface RuntimeStatus {
  collecting: boolean;
  paused: boolean;
  current_account: string;
  last_run_at: number | null;
  next_run_at: number | null;
  accounts_total: number;
  accounts_ok: number;
  works_total: number;
  snapshots_total: number;
  monitored_total: number;
  alerts_today: number;
  feishu_configured: boolean;
  feishu_enabled: boolean;
  chrome_path: string;
  data_dir: string;
  version: string;
}

export interface LogEntry {
  ts: number;
  level: string;
  target: string;
  message: string;
}

export interface CollectOutcome {
  account_id: number;
  account_name: string;
  ok: boolean;
  works_found: number;
  works_saved: number;
  new_works: number;
  snapshots: number;
  login_state: string;
  message: string;
  duration_ms: number;
}

export interface ExtractPreview {
  ok: boolean;
  blocked: boolean;
  reason: string;
  page_url: string;
  page_title: string;
  works: number;
  sample: Array<{
    aweme_id: string;
    title: string;
    author_name: string;
    url: string;
    published_at: number | null;
    metrics: Metrics;
    is_approximate: boolean;
  }>;
  debug: Record<string, unknown>;
}

export interface NotifyResult {
  ok: boolean;
  code: number;
  msg: string;
}

export interface LoginCheck {
  login_state: string;
  message: string;
}

export interface ComponentStatus {
  ok: boolean;
  name: string;
  version: string | null;
  path: string | null;
  detail: string;
  download_url: string | null;
}

export interface NetworkStatus {
  ok: boolean;
  douyin_main_ok: boolean;
  douyin_main_latency_ms: number | null;
  douyin_short_ok: boolean;
  douyin_short_latency_ms: number | null;
  feishu_ok: boolean | null;
  feishu_latency_ms: number | null;
  detail: string;
}

export interface StorageHealthStatus {
  ok: boolean;
  writable: boolean;
  data_dir: string;
  free_bytes: number;
  total_bytes: number;
  db_size_bytes: number;
  db_ok: boolean;
  detail: string;
}

export interface OsInfo {
  os_name: string;
  arch: string;
}

export interface EnvironmentReport {
  webview2: ComponentStatus;
  chrome: ComponentStatus;
  network: NetworkStatus;
  storage: StorageHealthStatus;
  os: OsInfo;
  all_passed: boolean;
  score: number;
  summary: string;
}
