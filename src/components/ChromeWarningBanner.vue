<script setup lang="ts">
import { computed, ref } from "vue";
import { api } from "@/api";
import { notify, refreshEnvironment, refreshStatus, store } from "@/store";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { AlertTriangle, ExternalLink, HardDrive, RefreshCw, Settings as SettingsIcon, WifiOff } from "lucide-vue-next";

const emit = defineEmits<{
  (e: "goto-settings"): void;
}>();

const checking = ref(false);

const CHROME_DOWNLOAD_URL = "https://www.google.cn/chrome/";
const WEBVIEW2_DOWNLOAD_URL = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

const report = computed(() => store.envReport);
const hasChrome = computed(() => (!report.value ? !!store.status?.chrome_path : report.value.chrome.ok));
const hasWebview2 = computed(() => report.value?.webview2.ok ?? true);
const networkOk = computed(() => report.value?.network.ok ?? true);
const storageOk = computed(() => report.value?.storage.ok ?? true);

// 综合异常按严重程度优先级排序
const activeIssue = computed(() => {
  if (!hasChrome.value) {
    return {
      kind: "chrome",
      isDestructive: true,
      title: "未检测到 Google Chrome 浏览器",
      desc: "本应用严格依赖 Google Chrome 执行后台无头采集与安全会话交互，缺少 Chrome 将无法采集作品数据。",
      actionText: "下载 Chrome",
      actionUrl: CHROME_DOWNLOAD_URL,
      settingsText: "手动指定路径",
    };
  }
  if (!hasWebview2.value) {
    return {
      kind: "webview2",
      isDestructive: true,
      title: "未检测到 Microsoft Edge WebView2 运行时",
      desc: "WebView2 是客户端 UI 渲染与安全交互的基础组件，若未正确安装可能导致窗口渲染空白或异常退出。",
      actionText: "下载 WebView2",
      actionUrl: WEBVIEW2_DOWNLOAD_URL,
      settingsText: "查看诊断",
    };
  }
  if (!networkOk.value) {
    return {
      kind: "network",
      isDestructive: false,
      title: "网络与抖音接口连通异常",
      desc: report.value?.network.detail || "无法正常连通抖音官方服务，请检查网络连接、系统代理或 DNS 设置。",
      actionText: "重新测试",
      actionUrl: "",
      settingsText: "前往诊断",
    };
  }
  if (!storageOk.value) {
    return {
      kind: "storage",
      isDestructive: false,
      title: "磁盘存储空间不足或读写受限",
      desc: report.value?.storage.detail || "本地数据目录磁盘空间紧张或缺少权限，可能导致快照数据无法写入。",
      actionText: "清理存储",
      actionUrl: "",
      settingsText: "存储设置",
    };
  }
  return null;
});

async function handleAction(): Promise<void> {
  if (!activeIssue.value) return;
  if (activeIssue.value.actionUrl) {
    try {
      await api.openUrl(activeIssue.value.actionUrl);
    } catch (err) {
      notify(`打开下载链接失败：${String(err)}`, "error");
    }
  } else if (activeIssue.value.kind === "network") {
    await recheck();
  } else if (activeIssue.value.kind === "storage") {
    emit("goto-settings");
  }
}

async function recheck(): Promise<void> {
  checking.value = true;
  try {
    const [found, rep] = await Promise.all([
      api.recheckChrome(),
      refreshEnvironment(),
      refreshStatus(),
    ]);
    if (rep?.all_passed) {
      notify("全项环境检测已通过，所有组件运行良好", "ok");
    } else if (found) {
      notify(`已检测到 Chrome，但仍有部分环境需注意：${rep?.summary}`, "warn");
    } else {
      notify("未检测到 Google Chrome，请前往官网下载或手动配置路径", "error");
    }
  } catch (err) {
    notify(`体检失败：${String(err)}`, "error");
  } finally {
    checking.value = false;
  }
}
</script>

<template>
  <Alert
    v-if="activeIssue"
    :variant="activeIssue.isDestructive ? 'destructive' : 'warning'"
    class="mb-4"
    :class="
      activeIssue.isDestructive
        ? 'bg-red-950/40 border-red-500/30 text-red-200'
        : 'bg-amber-950/40 border-amber-500/30 text-amber-200'
    "
  >
    <AlertTriangle
      class="h-4 w-4"
      :class="activeIssue.isDestructive ? 'text-red-400' : 'text-amber-400'"
    />
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
      <div>
        <AlertTitle
          class="font-semibold text-sm"
          :class="activeIssue.isDestructive ? 'text-red-300' : 'text-amber-300'"
        >
          {{ activeIssue.title }}
        </AlertTitle>
        <AlertDescription class="text-slate-300 text-xs mt-1">
          {{ activeIssue.desc }}
          <template v-if="activeIssue.actionUrl">
            请前往
            <a
              :href="activeIssue.actionUrl"
              class="text-blue-400 underline hover:text-blue-300 inline-flex items-center gap-0.5 font-medium cursor-pointer"
              @click.prevent="handleAction"
            >
              官方源地址 ({{ activeIssue.actionUrl }})
            </a>
            下载安装。
          </template>
        </AlertDescription>
      </div>

      <div class="flex items-center gap-2 shrink-0">
        <Button
          size="sm"
          :variant="activeIssue.isDestructive ? 'destructive' : 'default'"
          @click="handleAction"
        >
          <ExternalLink v-if="activeIssue.actionUrl" class="w-3.5 h-3.5" />
          <WifiOff v-else-if="activeIssue.kind === 'network'" class="w-3.5 h-3.5" />
          <HardDrive v-else-if="activeIssue.kind === 'storage'" class="w-3.5 h-3.5" />
          <span>{{ activeIssue.actionText }}</span>
        </Button>
        <Button
          size="sm"
          variant="outline"
          class="hover:bg-slate-800 text-slate-200"
          :class="activeIssue.isDestructive ? 'border-red-500/30' : 'border-amber-500/30'"
          @click="emit('goto-settings')"
        >
          <SettingsIcon class="w-3.5 h-3.5" />
          <span>{{ activeIssue.settingsText }}</span>
        </Button>
        <Button
          size="sm"
          variant="secondary"
          :disabled="checking"
          @click="recheck"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': checking }" />
          <span>{{ checking ? "检测中..." : "重新检测" }}</span>
        </Button>
      </div>
    </div>
  </Alert>
</template>
