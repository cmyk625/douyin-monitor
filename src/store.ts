import { reactive } from "vue";
import { api, onEvent } from "./api";
import { loginStateText } from "./utils";
import type {
  Account,
  Alert,
  EnvironmentReport,
  LogEntry,
  MonitoredWork,
  Rule,
  RuntimeStatus,
  Settings,
  Work,
} from "./types";

export interface Store {
  ready: boolean;
  initError: string | null;
  status: RuntimeStatus | null;
  envReport: EnvironmentReport | null;
  envChecking: boolean;
  accounts: Account[];
  works: Work[];
  monitored: MonitoredWork[];
  rules: Rule[];
  alerts: Alert[];
  settings: Settings | null;
  logs: LogEntry[];
  workFilter: { accountId: number | null; keyword: string; limit: number };
  loading: Record<string, boolean>;
  toast: { text: string; kind: "info" | "ok" | "error" | "warn" } | null;
}

export const store = reactive<Store>({
  ready: false,
  initError: null,
  status: null,
  envReport: null,
  envChecking: false,
  accounts: [],
  works: [],
  monitored: [],
  rules: [],
  alerts: [],
  settings: null,
  logs: [],
  workFilter: { accountId: null, keyword: "", limit: 200 },
  loading: {},
  toast: null,
});

let toastTimer: number | undefined;

export function notify(text: string, kind: "info" | "ok" | "error" | "warn" = "info"): void {
  store.toast = { text, kind };
  if (toastTimer) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    store.toast = null;
  }, 3600);
}

export function setLoading(key: string, value: boolean): void {
  store.loading[key] = value;
}

export async function refreshStatus(): Promise<void> {
  store.status = await api.getStatus();
}

export async function refreshAccounts(): Promise<void> {
  store.accounts = await api.listAccounts();
}

export async function refreshWorks(): Promise<void> {
  const { accountId, keyword, limit } = store.workFilter;
  store.works = await api.listWorks(accountId, keyword, limit, true);
}

export async function refreshMonitored(): Promise<void> {
  store.monitored = await api.listMonitored();
}

export async function refreshRules(): Promise<void> {
  store.rules = await api.listRules();
}

export async function refreshAlerts(): Promise<void> {
  store.alerts = await api.listAlerts(200);
}

export async function refreshSettings(): Promise<void> {
  store.settings = await api.getSettings();
}

export async function refreshLogs(): Promise<void> {
  store.logs = await api.listLogs(300);
}

export async function refreshEnvironment(): Promise<EnvironmentReport | null> {
  store.envChecking = true;
  try {
    const report = await api.checkEnvironment();
    store.envReport = report;
    return report;
  } catch (err) {
    notify(`环境体检失败：${String(err)}`, "error");
    return null;
  } finally {
    store.envChecking = false;
  }
}

/** 首次加载：单个接口失败不会让界面卡在加载态，但会把错误显示出来。 */
export async function refreshAll(): Promise<void> {
  const tasks: Array<[string, Promise<unknown>]> = [
    ["运行状态", refreshStatus()],
    ["环境体检", refreshEnvironment()],
    ["账号列表", refreshAccounts()],
    ["监控视频", refreshMonitored()],
    ["报警规则", refreshRules()],
    ["通知记录", refreshAlerts()],
    ["设置", refreshSettings()],
    ["运行日志", refreshLogs()],
  ];
  const results = await Promise.allSettled(tasks.map(([, task]) => task));
  const failures = results
    .map((result, index) =>
      result.status === "rejected" ? `${tasks[index][0]}：${describeError(result.reason)}` : null,
    )
    .filter((item): item is string => item !== null);
  store.initError = failures.length > 0 ? failures.join("\n") : null;
  store.ready = true;
}

export function describeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

/** 注册后端事件（只需在应用启动时调用一次），返回取消监听函数 */
export async function bindEvents(): Promise<Array<() => void>> {
  const unlisten: Array<() => void> = [];

  unlisten.push(
    await onEvent("log", (entry) => {
      store.logs.unshift(entry);
      if (store.logs.length > 400) store.logs.pop();
    }),
  );

  unlisten.push(
    await onEvent("status", (status) => {
      store.status = status;
    }),
  );

  unlisten.push(
    await onEvent("collect:started", (payload) => {
      if (store.status) {
        store.status.collecting = true;
        store.status.current_account = payload.account_name;
      }
    }),
  );

  unlisten.push(
    await onEvent("collect:finished", async (outcome) => {
      if (store.status) {
        store.status.collecting = false;
        store.status.current_account = "";
      }
      await Promise.all([refreshAccounts(), refreshWorks(), refreshMonitored(), refreshAlerts(), refreshStatus()]);
      if (!outcome.ok) {
        notify(`${outcome.account_name}：${outcome.message}`, "error");
      } else if (outcome.new_works > 0) {
        notify(`${outcome.account_name}：发现 ${outcome.new_works} 个新作品`, "ok");
      }
    }),
  );

  unlisten.push(
    await onEvent("alert:created", async (alerts) => {
      await Promise.all([refreshAlerts(), refreshStatus()]);
      for (const alert of alerts) {
        notify(`${alert.rule_name}：${alert.work_title} ${alert.delta > 0 ? "+" : ""}${alert.delta}`, "ok");
      }
    }),
  );

  unlisten.push(
    await onEvent("login:changed", async (payload) => {
      // 后端在登录窗口打开期间持续监控，确认成功后在这里刷新列表并提示。
      const name =
        store.accounts.find((item) => item.id === payload.account_id)?.name ?? `#${payload.account_id}`;
      await Promise.all([refreshAccounts(), refreshStatus()]);
      const ok = payload.login_state === "ok";
      notify(
        ok
          ? `「${name}」登录成功，已加入定时采集`
          : `「${name}」${loginStateText(payload.login_state)} · ${payload.message}`,
        ok ? "ok" : "error",
      );
    }),
  );

  unlisten.push(
    await onEvent("notify:result", (result) => {
      notify(result.ok ? "飞书通知已发送" : `飞书通知失败：${result.msg}`, result.ok ? "ok" : "error");
    }),
  );

  return unlisten;
}
