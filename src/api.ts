import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Account,
  AccountInput,
  Alert,
  CleanStorageResult,
  CollectOutcome,
  EnvironmentReport,
  ExtractPreview,
  FeishuConfig,
  LoginCheck,
  LogEntry,
  MonitoredInput,
  MonitoredWork,
  NotifyResult,
  Rule,
  RuleInput,
  RuntimeStatus,
  Settings,
  SnapshotPoint,
  StorageStats,
  Work,
} from "./types";

export const api = {
  listAccounts: () => invoke<Account[]>("list_accounts"),
  createAccount: (input: AccountInput) => invoke<Account>("create_account", { input }),
  updateAccount: (input: { id: number; name: string }) => invoke<Account>("update_account", { input }),
  deleteAccount: (id: number) => invoke<void>("delete_account", { id }),
  openLoginWindow: (id: number) => invoke<void>("open_login_window", { id }),
  checkLogin: (id: number) => invoke<LoginCheck>("check_login", { id }),
  collectNow: (id: number) => invoke<CollectOutcome[]>("collect_now", { id }),
  collectAll: () => invoke<CollectOutcome[]>("collect_all"),
  recheckChrome: () => invoke<string | null>("recheck_chrome"),
  previewExtract: (id: number) => invoke<ExtractPreview>("preview_extract", { id }),

  listMonitored: () => invoke<MonitoredWork[]>("list_monitored"),
  saveMonitored: (input: MonitoredInput) => invoke<MonitoredWork>("save_monitored", { input }),
  deleteMonitored: (id: number) => invoke<void>("delete_monitored", { id }),

  listWorks: (accountId: number | null, keyword: string, limit: number, history = false, offset = 0) =>
    invoke<Work[]>("list_works", { accountId, keyword, limit, history, offset }),
  workTrend: (workId: number, limit: number) =>
    invoke<SnapshotPoint[]>("work_trend", { workId, limit }),
  deleteWork: (id: number) => invoke<void>("delete_work", { id }),

  listRules: () => invoke<Rule[]>("list_rules"),
  saveRule: (input: RuleInput) => invoke<Rule>("save_rule", { input }),
  deleteRule: (id: number) => invoke<void>("delete_rule", { id }),
  runRules: () => invoke<number>("run_rules"),

  listAlerts: (limit: number) => invoke<Alert[]>("list_alerts", { limit }),
  clearAlerts: () => invoke<void>("clear_alerts"),
  resendAlert: (id: number) => invoke<NotifyResult>("resend_alert", { id }),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (input: Settings) => invoke<Settings>("save_settings", { input }),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  getFeishuConfig: () => invoke<FeishuConfig>("get_feishu_config"),
  saveFeishuConfig: (input: FeishuConfig) => invoke<FeishuConfig>("save_feishu_config", { input }),
  testFeishu: (config?: FeishuConfig) => invoke<NotifyResult>("test_feishu", { config }),

  getStatus: () => invoke<RuntimeStatus>("get_status"),
  setPaused: (paused: boolean) => invoke<RuntimeStatus>("set_paused", { paused }),
  listLogs: (limit: number) => invoke<LogEntry[]>("list_logs", { limit }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openDataDir: () => invoke<void>("open_data_dir"),
  openConfigFile: () => invoke<void>("open_config_file"),
  getStorageStats: () => invoke<StorageStats>("get_storage_stats"),
  cleanStorage: (retentionDays?: number) =>
    invoke<CleanStorageResult>("clean_storage", { retentionDays }),
  checkEnvironment: () => invoke<EnvironmentReport>("check_environment"),
};

export interface AppEvents {
  "collect:started": { account_id: number; account_name: string };
  "collect:finished": CollectOutcome;
  "alert:created": Alert[];
  "notify:result": NotifyResult;
  "login:changed": { account_id: number; login_state: string; message: string };
  "log": LogEntry;
  "status": RuntimeStatus;
}

export function onEvent<K extends keyof AppEvents>(
  name: K,
  handler: (payload: AppEvents[K]) => void,
): Promise<UnlistenFn> {
  return listen<AppEvents[K]>(name, (event) => handler(event.payload));
}
