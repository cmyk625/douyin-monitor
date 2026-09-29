<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { api } from "@/api";
import { notify, refreshAlerts, setLoading, store } from "@/store";
import { fmtDelta, fmtNum, fmtTime, metricLabel } from "@/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import Pagination from "@/components/Pagination.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableEmpty,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ExternalLink, RefreshCw, Trash2 } from "lucide-vue-next";
import type { Alert } from "@/types";

const detail = ref<Alert | null>(null);

const pageSize = ref(15);
const currentPage = ref(1);

const paginatedAlerts = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value;
  return store.alerts.slice(start, start + pageSize.value);
});

watch(
  () => store.alerts.length,
  (len) => {
    const maxPage = Math.max(1, Math.ceil(len / pageSize.value));
    if (currentPage.value > maxPage) {
      currentPage.value = maxPage;
    }
  },
);

async function resend(alert: Alert): Promise<void> {
  setLoading(`alert-${alert.id}`, true);
  try {
    const result = await api.resendAlert(alert.id);
    if (result.ok) {
      notify("已重新推送飞书通知", "ok");
    } else {
      notify(`推送失败：${result.msg}`, "error");
    }
    await refreshAlerts();
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading(`alert-${alert.id}`, false);
  }
}

async function clearAll(): Promise<void> {
  if (!window.confirm("确定清空全部通知历史记录？")) return;
  try {
    await api.clearAlerts();
    currentPage.value = 1;
    await refreshAlerts();
    notify("通知记录已清空", "ok");
  } catch (err) {
    notify(String(err), "error");
  }
}

function openLink(url: string): void {
  if (!url) return;
  void api.openUrl(url).catch((err) => notify(String(err), "error"));
}

onMounted(() => {
  void refreshAlerts();
});
</script>

<template>
  <div class="h-full flex flex-col min-h-0 space-y-3">
    <div class="flex items-end justify-between gap-4 pb-1 shrink-0">
      <div>
        <h1 class="text-xl font-bold tracking-tight text-white">通知记录</h1>
        <p class="text-xs text-muted-foreground mt-1">每次规则命中触发报警时的详细快照与飞书推送结果记录。</p>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" @click="refreshAlerts">
          <RefreshCw class="w-3.5 h-3.5" />
          <span>刷新</span>
        </Button>
        <Button variant="destructive" size="sm" @click="clearAll">
          <Trash2 class="w-3.5 h-3.5" />
          <span>清空记录</span>
        </Button>
      </div>
    </div>

    <!-- 通知记录表格 (shadcn Table，表格内容内部滚动) -->
    <Table containerClass="flex-1 min-h-0 overflow-auto">
      <TableHeader>
        <TableRow>
          <TableHead>触发时间</TableHead>
          <TableHead>规则名称</TableHead>
          <TableHead>采集账号</TableHead>
          <TableHead>作者</TableHead>
          <TableHead>作品标题</TableHead>
          <TableHead>指标</TableHead>
          <TableHead class="text-right">起点数值</TableHead>
          <TableHead class="text-right">当前数值</TableHead>
          <TableHead class="text-right">增量</TableHead>
          <TableHead>推送状态</TableHead>
          <TableHead class="w-1 text-center">操作</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="alert in paginatedAlerts" :key="alert.id">
          <TableCell class="font-mono text-muted-foreground whitespace-nowrap text-[11px]">
            {{ fmtTime(alert.created_at) }}
          </TableCell>
          <TableCell class="font-medium text-white">{{ alert.rule_name }}</TableCell>
          <TableCell class="text-muted-foreground whitespace-nowrap text-xs">{{ alert.account_name }}</TableCell>
          <TableCell class="text-slate-300">{{ alert.author_name || "—" }}</TableCell>
          <TableCell class="truncate max-w-[220px] text-slate-300" :title="alert.work_title">
            {{ alert.work_title }}
          </TableCell>
          <TableCell class="whitespace-nowrap">
            <Badge variant="secondary" class="font-mono text-[11px]">
              {{ metricLabel(alert.metric) }}
            </Badge>
          </TableCell>
          <TableCell class="text-right font-mono text-muted-foreground whitespace-nowrap">{{ fmtNum(alert.baseline_value) }}</TableCell>
          <TableCell class="text-right font-mono text-white whitespace-nowrap">{{ fmtNum(alert.current_value) }}</TableCell>
          <TableCell class="text-right font-mono font-semibold text-emerald-400 whitespace-nowrap">
            {{ fmtDelta(alert.delta) }}
          </TableCell>
          <TableCell class="whitespace-nowrap">
            <Badge
              :variant="alert.notify_state === 'sent' ? 'success' : alert.notify_state === 'failed' ? 'destructive' : 'secondary'"
              :title="alert.notify_detail"
            >
              {{ alert.notify_state === "sent" ? "已发送" : alert.notify_state === "failed" ? "失败" : "未发送" }}
            </Badge>
          </TableCell>
          <TableCell>
            <div class="flex items-center justify-end gap-1.5">
              <Button size="sm" variant="ghost" class="h-7 text-xs px-2" @click="detail = alert">详情</Button>
              <Button
                size="sm"
                variant="outline"
                class="h-7 text-xs px-2"
                :disabled="store.loading[`alert-${alert.id}`]"
                @click="resend(alert)"
              >
                重发
              </Button>
            </div>
          </TableCell>
        </TableRow>

        <TableEmpty v-if="store.alerts.length === 0" :colspan="11">
          暂无通知记录。
        </TableEmpty>
      </TableBody>
    </Table>

    <!-- 分页导航条（固定在底部，无需页面滚动） -->
    <Pagination
      v-if="store.alerts.length > 0"
      v-model:current-page="currentPage"
      v-model:page-size="pageSize"
      :total-items="store.alerts.length"
      class="shrink-0 pt-1 border-t border-border/40"
    />

    <!-- 报警详情弹窗 (shadcn Dialog, 右上角固定关闭按钮) -->
    <Dialog :open="!!detail" @update:open="(val) => { if (!val) detail = null; }">
      <DialogContent class="sm:max-w-lg max-h-[90vh] flex flex-col p-0 overflow-hidden">
        <DialogHeader class="p-6 pb-3 pr-14 border-b border-border/40 shrink-0">
          <DialogTitle>报警记录详情</DialogTitle>
          <DialogDescription>
            命中规则时的上下文与飞书消息载荷。
          </DialogDescription>
        </DialogHeader>

        <div v-if="detail" class="flex-1 overflow-y-auto p-6 space-y-4 text-xs">
          <div class="grid grid-cols-2 gap-2 text-slate-300 bg-muted/20 p-3 rounded-lg border border-border/50">
            <div>
              <span class="text-muted-foreground">触发时间：</span>
              <span class="font-mono text-white">{{ fmtTime(detail.created_at) }}</span>
            </div>
            <div>
              <span class="text-muted-foreground">规则名称：</span>
              <span class="text-white font-medium">{{ detail.rule_name }}</span>
            </div>
            <div>
              <span class="text-muted-foreground">采集账号：</span>
              <span class="text-slate-200">{{ detail.account_name }}</span>
            </div>
            <div>
              <span class="text-muted-foreground">数据增长：</span>
              <span class="font-mono text-emerald-400 font-semibold">
                {{ fmtNum(detail.baseline_value) }} → {{ fmtNum(detail.current_value) }} ({{ fmtDelta(detail.delta) }})
              </span>
            </div>
            <div class="col-span-2 flex items-center gap-1.5">
              <span class="text-muted-foreground">作品标题：</span>
              <button
                class="text-blue-400 hover:underline flex items-center gap-1 cursor-pointer truncate max-w-xs"
                @click="openLink(detail.work_url)"
              >
                <span>{{ detail.work_title }}</span>
                <ExternalLink class="w-3 h-3 shrink-0" />
              </button>
            </div>
            <div class="col-span-2">
              <span class="text-muted-foreground">推送状态：</span>
              <Badge
                :variant="detail.notify_state === 'sent' ? 'success' : detail.notify_state === 'failed' ? 'destructive' : 'secondary'"
              >
                {{ detail.notify_state === "sent" ? "已送达" : detail.notify_state === "failed" ? "发送失败" : "未发送" }}
              </Badge>
              <span class="text-muted-foreground text-[11px] ml-2">{{ detail.notify_detail || "无详情" }}</span>
            </div>
          </div>

          <div class="space-y-1">
            <label class="text-xs font-medium text-slate-300">飞书推送原始文本</label>
            <pre class="font-mono text-xs text-slate-200 bg-background/80 p-3 rounded-md border border-border/60 whitespace-pre-wrap word-break-break-all">{{ detail.message }}</pre>
          </div>
        </div>

        <DialogFooter class="p-4 px-6 border-t border-border/40 shrink-0">
          <Button variant="ghost" size="sm" @click="detail = null">关闭</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
