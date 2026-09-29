<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { api } from "@/api";
import { notify, store } from "@/store";
import { fmtTime } from "@/utils";
import { Button } from "@/components/ui/button";
import { Copy, RefreshCw } from "lucide-vue-next";

const autoRefresh = ref(true);
let timer: number | undefined;

async function refresh(): Promise<void> {
  try {
    store.logs = await api.listLogs(300);
  } catch (err) {
    notify(String(err), "error");
  }
}

async function copyAll(): Promise<void> {
  const text = [...store.logs]
    .reverse()
    .map((line) => `[${fmtTime(line.ts)}] ${line.level} ${line.target} ${line.message}`)
    .join("\n");
  try {
    await navigator.clipboard.writeText(text);
    notify("日志已复制到剪贴板", "ok");
  } catch (err) {
    notify(`复制失败：${String(err)}`, "error");
  }
}

onMounted(async () => {
  await refresh();
  timer = window.setInterval(() => {
    if (autoRefresh.value) void refresh();
  }, 3000);
});

onUnmounted(() => {
  if (timer) window.clearInterval(timer);
});
</script>

<template>
  <div class="h-full flex flex-col min-h-0 space-y-4">
    <div class="flex items-end justify-between gap-4 pb-1 shrink-0">
      <div>
        <h1 class="text-xl font-bold tracking-tight text-white">运行日志</h1>
        <p class="text-xs text-muted-foreground mt-1">实时展示后台自动化采集调度、会话状态与网络推送事件（最近 300 条）。</p>
      </div>
      <div class="flex items-center gap-2">
        <label class="flex items-center gap-2 cursor-pointer text-xs text-slate-300 mr-2">
          <input v-model="autoRefresh" type="checkbox" class="rounded border-border" />
          <span>自动刷新</span>
        </label>
        <Button variant="outline" size="sm" @click="refresh">
          <RefreshCw class="w-3.5 h-3.5" />
          <span>刷新</span>
        </Button>
        <Button variant="secondary" size="sm" @click="copyAll">
          <Copy class="w-3.5 h-3.5" />
          <span>复制全部</span>
        </Button>
      </div>
    </div>

    <!-- 日志展示容器 -->
    <div class="logs flex-1 min-h-0 overflow-y-auto">
      <div v-for="(line, index) in store.logs" :key="`${line.ts}-${index}`" class="log-line">
        <span class="muted">{{ fmtTime(line.ts).slice(11) }}</span>
        <span class="log-level" :class="line.level">{{ line.level }}</span>
        <span class="text-slate-200">{{ line.message }}</span>
      </div>
      <div v-if="store.logs.length === 0" class="empty">暂无运行日志。</div>
    </div>
  </div>
</template>
