<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api } from "@/api";
import { bindEvents, notify, refreshAll, refreshStatus, store } from "@/store";
import { fmtAgo } from "@/utils";
import { Button } from "@/components/ui/button";
import ChromeWarningBanner from "@/components/ChromeWarningBanner.vue";
import DashboardView from "@/views/DashboardView.vue";
import AccountsView from "@/views/AccountsView.vue";
import WorksView from "@/views/WorksView.vue";
import RulesView from "@/views/RulesView.vue";
import AlertsView from "@/views/AlertsView.vue";
import SettingsView from "@/views/SettingsView.vue";
import LogsView from "@/views/LogsView.vue";
import GuideView from "@/views/GuideView.vue";
import {
  Activity,
  AlertCircle,
  AlertTriangle,
  Bell,
  BookOpen,
  CheckCircle2,
  LayoutDashboard,
  Settings,
  ShieldAlert,
  Terminal,
  Users,
  Video,
} from "lucide-vue-next";

type TabKey = "dashboard" | "accounts" | "works" | "rules" | "alerts" | "settings" | "logs" | "guide";

const tabs: Array<{ key: TabKey; label: string; icon: any }> = [
  { key: "dashboard", label: "系统概览", icon: LayoutDashboard },
  { key: "accounts", label: "采集账号", icon: Users },
  { key: "works", label: "监控作品", icon: Video },
  { key: "rules", label: "报警规则", icon: ShieldAlert },
  { key: "alerts", label: "通知记录", icon: Bell },
  { key: "settings", label: "系统设置", icon: Settings },
  { key: "logs", label: "运行日志", icon: Terminal },
  { key: "guide", label: "使用教程", icon: BookOpen },
];

const tab = ref<TabKey>("dashboard");
const collecting = computed(() => store.status?.collecting ?? false);
const paused = computed(() => store.status?.paused ?? false);
const accountsOk = computed(() => store.accounts.filter((a) => a.login_state === "ok").length);
const hasChrome = computed(() => !!store.status?.chrome_path);

const envAllPassed = computed(() => {
  if (store.envReport) {
    return store.envReport.all_passed;
  }
  return hasChrome.value;
});

const envStatusText = computed(() => {
  if (store.envChecking) return "检测中...";
  if (store.envReport) {
    return store.envReport.all_passed ? "良好" : "需注意";
  }
  return hasChrome.value ? "就绪" : "异常";
});

const envStatusClass = computed(() => {
  if (store.envChecking) return "text-blue-400";
  return envAllPassed.value ? "text-emerald-400" : "text-amber-400";
});

const envStatusTitle = computed(() => {
  if (store.envReport) {
    return `综合体检得分：${store.envReport.score}分 - ${store.envReport.summary}（点击进入诊断）`;
  }
  return hasChrome.value ? "环境已就绪" : "未检测到 Chrome，点击前往配置或下载";
});

let pollTimer: number | undefined;
let unlisten: Array<() => void> = [];
let eventsBound = false;

const LOAD_TIMEOUT_MS = 15_000;

async function loadAll(): Promise<void> {
  store.ready = false;
  store.initError = null;
  await Promise.race([
    refreshAll(),
    new Promise<void>((resolve) => window.setTimeout(resolve, LOAD_TIMEOUT_MS)),
  ]);
  if (!store.ready) {
    store.initError = `后端命令 ${LOAD_TIMEOUT_MS / 1000} 秒内没有返回，请点击重试；若仍失败请重启应用。`;
    store.ready = true;
  }
  await bindEventsOnce();
}

async function bindEventsOnce(): Promise<void> {
  if (eventsBound) return;
  eventsBound = true;
  try {
    unlisten = await bindEvents();
  } catch (err) {
    notify(`事件监听注册失败：${String(err)}`, "error");
  }
}

function countsFor(key: TabKey): string {
  switch (key) {
    case "accounts":
      return String(store.accounts.length);
    case "works":
      return String(store.status?.monitored_total ?? store.monitored.length);
    case "rules":
      return String(store.rules.length);
    case "alerts":
      return String(store.alerts.length);
    default:
      return "";
  }
}

async function togglePause(): Promise<void> {
  try {
    const status = await api.setPaused(!paused.value);
    store.status = status;
    notify(status.paused ? "已暂停自动采集调度" : "已恢复自动采集调度", "info");
  } catch (err) {
    notify(String(err), "error");
  }
}

onMounted(async () => {
  await loadAll();
  pollTimer = window.setInterval(() => {
    void refreshStatus().catch(() => undefined);
  }, 10_000);
});

onUnmounted(() => {
  if (pollTimer) window.clearInterval(pollTimer);
  for (const fn of unlisten) fn();
});
</script>

<template>
  <div class="app">
    <!-- 侧边导航栏 (shadcn 质感风格) -->
    <aside class="sidebar">
      <div class="brand">
        <span class="brand-dot" :class="collecting ? 'bg-amber-400 shadow-amber-400' : 'bg-emerald-500 shadow-emerald-500'" />
        <span class="brand-name">抖音作品监控</span>
      </div>

      <nav class="space-y-1">
        <button
          v-for="item in tabs"
          :key="item.key"
          class="nav-item w-full"
          :class="{ active: tab === item.key }"
          @click="tab = item.key"
        >
          <div class="nav-item-inner">
            <component :is="item.icon" class="w-4 h-4 shrink-0" />
            <span>{{ item.label }}</span>
          </div>
          <span v-if="countsFor(item.key)" class="nav-count">{{ countsFor(item.key) }}</span>
        </button>
      </nav>

      <div class="sidebar-foot">
        <div class="sidebar-chip flex items-center justify-between">
          <div class="flex items-center gap-1.5">
            <span
              class="w-1.5 h-1.5 rounded-full"
              :class="paused ? 'bg-amber-400' : 'bg-emerald-400'"
            />
            <span>{{ paused ? "已暂停调度" : "自动调度中" }}</span>
          </div>
          <span v-if="collecting" class="pulse" title="正在采集中" />
        </div>

        <div
          class="sidebar-chip cursor-pointer hover:border-slate-500 transition-colors flex items-center gap-1.5"
          :title="envStatusTitle"
          @click="tab = 'settings'"
        >
          <Activity class="w-3.5 h-3.5 shrink-0" :class="envStatusClass" />
          <span class="truncate">环境: {{ envStatusText }}</span>
        </div>

        <div class="text-[11px] text-muted-foreground px-1 space-y-0.5">
          <div>账号 {{ accountsOk }}/{{ store.accounts.length }} 已登录</div>
          <div>上次采集 {{ fmtAgo(store.status?.last_run_at ?? null) }}</div>
        </div>
      </div>
    </aside>

    <!-- 主工作区 -->
    <main class="main">
      <!-- 环境异常预警横幅（自动感知 Chrome/WebView2/网络/存储状态） -->
      <ChromeWarningBanner v-if="store.ready" class="shrink-0 mb-3" @goto-settings="tab = 'settings'" />

      <!-- 通信错误提示 -->
      <div v-if="store.initError" class="p-4 rounded-xl border border-red-500/40 bg-red-950/30 mb-4 shrink-0">
        <div class="flex items-center gap-2 text-red-400 font-semibold text-sm mb-1.5">
          <AlertCircle class="w-4 h-4" />
          <span>后端通信异常</span>
        </div>
        <pre class="font-mono text-xs text-muted-foreground whitespace-pre-wrap word-break-break-all mb-3">{{ store.initError }}</pre>
        <div class="flex items-center gap-2">
          <Button size="sm" @click="loadAll">重试连接</Button>
          <Button variant="ghost" size="sm" @click="store.initError = null">忽略</Button>
        </div>
      </div>

      <!-- 首次加载顶部进度指示 -->
      <div v-if="!store.ready" class="h-0.5 w-full bg-blue-500/20 overflow-hidden mb-3 rounded-full shrink-0">
        <div class="h-full bg-blue-500 animate-pulse w-1/3" />
      </div>

      <!-- 视图内容 -->
      <DashboardView v-if="tab === 'dashboard'" class="flex-1 min-h-0" @goto="tab = $event" @toggle-pause="togglePause" />
      <AccountsView v-else-if="tab === 'accounts'" class="flex-1 min-h-0" @goto-works="tab = 'works'" />
      <WorksView v-else-if="tab === 'works'" class="flex-1 min-h-0" />
      <RulesView v-else-if="tab === 'rules'" class="flex-1 min-h-0" />
      <AlertsView v-else-if="tab === 'alerts'" class="flex-1 min-h-0" />
      <SettingsView v-else-if="tab === 'settings'" class="flex-1 min-h-0" />
      <LogsView v-else-if="tab === 'logs'" class="flex-1 min-h-0" />
      <GuideView v-else-if="tab === 'guide'" class="flex-1 min-h-0" @goto="tab = $event as TabKey" />
    </main>

    <!-- 浮动 Toast 消息通知 -->
    <div v-if="store.toast" class="toast" :class="store.toast.kind">
      <CheckCircle2 v-if="store.toast.kind === 'ok'" class="w-4 h-4 text-emerald-400 shrink-0" />
      <AlertTriangle v-else-if="store.toast.kind === 'error'" class="w-4 h-4 text-red-400 shrink-0" />
      <AlertCircle v-else-if="store.toast.kind === 'warn'" class="w-4 h-4 text-amber-400 shrink-0" />
      <AlertCircle v-else class="w-4 h-4 text-blue-400 shrink-0" />
      <span class="text-xs">{{ store.toast.text }}</span>
    </div>
  </div>
</template>
