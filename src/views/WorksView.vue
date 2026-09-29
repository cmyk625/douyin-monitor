<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { api } from "@/api";
import { notify, refreshMonitored, refreshStatus, setLoading, store } from "@/store";
import { extractDouyinUrl, fmtDelta, fmtNum, fmtPublishDate, fmtShort, fmtTime, metricLabel } from "@/utils";
import Sparkline from "@/components/Sparkline.vue";
import Pagination from "@/components/Pagination.vue";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  Plus,
  RefreshCw,
  Search,
  User,
  X,
} from "lucide-vue-next";
import type { MonitoredInput, MonitoredWork, SnapshotPoint } from "@/types";

type SortField = "like" | "comment" | "share" | "collect" | "published_at" | null;
type SortOrder = "asc" | "desc";

const editing = ref<MonitoredInput | null>(null);
const detail = ref<{ title: string; workId: number; trend: SnapshotPoint[]; metric: string } | null>(null);
const detailBusy = ref(false);
const collectAllBusy = ref(false);

const searchQuery = ref("");
const filterStatus = ref<string>("all");
const sortField = ref<SortField>(null);
const sortOrder = ref<SortOrder>("desc");

function toggleSort(field: "like" | "comment" | "share" | "collect" | "published_at"): void {
  if (sortField.value === field) {
    if (sortOrder.value === "desc") {
      sortOrder.value = "asc";
    } else {
      sortField.value = null;
      sortOrder.value = "desc";
    }
  } else {
    sortField.value = field;
    sortOrder.value = "desc";
  }
}

// 仅保留点赞、评论、分享、收藏（公开视频无播放量展示）
const metricKeys = ["like", "comment", "share", "collect"] as const;

const hasChrome = computed(() => !!store.status?.chrome_path);

const filterStatusLabel = computed(() => {
  if (filterStatus.value === "enabled") {
    return `监控中 (${store.monitored.filter((m) => m.enabled).length})`;
  }
  if (filterStatus.value === "disabled") {
    return `已停用 (${store.monitored.filter((m) => !m.enabled).length})`;
  }
  if (filterStatus.value === "error") {
    return `存在报错 (${store.monitored.filter((m) => !!m.last_error).length})`;
  }
  return `全部状态 (${store.monitored.length})`;
});

const currentEditingAccountName = computed(() => {
  if (!editing.value?.account_id) return "选择采集账号";
  const acc = store.accounts.find((a) => a.id === editing.value?.account_id);
  return acc ? `${acc.name} (${acc.login_state === "ok" ? "已登录" : "未登录"})` : "选择采集账号";
});

const filteredMonitored = computed(() => {
  let list = store.monitored;
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter(
      (item) =>
        (item.title && item.title.toLowerCase().includes(q)) ||
        (item.aweme_id && item.aweme_id.toLowerCase().includes(q)) ||
        (item.author_name && item.author_name.toLowerCase().includes(q)) ||
        (item.owner_name && item.owner_name.toLowerCase().includes(q)) ||
        (item.account_name && item.account_name.toLowerCase().includes(q)),
    );
  }
  if (filterStatus.value === "enabled") {
    list = list.filter((item) => item.enabled);
  } else if (filterStatus.value === "disabled") {
    list = list.filter((item) => !item.enabled);
  } else if (filterStatus.value === "error") {
    list = list.filter((item) => !!item.last_error);
  }
  if (sortField.value) {
    const field = sortField.value;
    const factor = sortOrder.value === "asc" ? 1 : -1;
    list = [...list].sort((a, b) => {
      let valA: number;
      let valB: number;
      if (field === "published_at") {
        valA = a.published_at ?? 0;
        valB = b.published_at ?? 0;
      } else {
        valA = a.metrics[field] ?? 0;
        valB = b.metrics[field] ?? 0;
      }
      return (valA - valB) * factor;
    });
  }
  return list;
});

const pageSize = ref(15);
const currentPage = ref(1);

const paginatedMonitored = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value;
  return filteredMonitored.value.slice(start, start + pageSize.value);
});

watch([searchQuery, filterStatus, sortField, sortOrder], () => {
  currentPage.value = 1;
});

watch(
  () => filteredMonitored.value.length,
  (len) => {
    const maxPage = Math.max(1, Math.ceil(len / pageSize.value));
    if (currentPage.value > maxPage) {
      currentPage.value = maxPage;
    }
  },
);

const blank = (): MonitoredInput => ({
  id: null,
  account_id: store.accounts[0]?.id ?? 0,
  target: "",
  owner_name: "",
  enabled: true,
});

const RECENT_OWNERS_KEY = "douyin_recent_owners";
const recentOwners = ref<string[]>([]);
const showOwnerPanel = ref(false);

function loadRecentOwners(): void {
  try {
    const raw = localStorage.getItem(RECENT_OWNERS_KEY);
    if (raw !== null) {
      const list = JSON.parse(raw);
      if (Array.isArray(list)) {
        recentOwners.value = list
          .filter((n): n is string => typeof n === "string" && !!n.trim())
          .slice(0, 10);
        return;
      }
    }
  } catch {}

  // 回退初始化：从当前已有的监控视频列表中提取最新保存的负责人（最多 8 个）
  const set = new Set<string>();
  const list: string[] = [];
  for (const item of store.monitored) {
    const name = item.owner_name?.trim();
    if (name && !set.has(name)) {
      set.add(name);
      list.push(name);
      if (list.length >= 8) break;
    }
  }
  recentOwners.value = list;
}

function recordRecentOwner(name: string): void {
  const trimmed = name.trim();
  if (!trimmed) return;
  const list = [trimmed, ...recentOwners.value.filter((n) => n !== trimmed)].slice(0, 10);
  recentOwners.value = list;
  try {
    localStorage.setItem(RECENT_OWNERS_KEY, JSON.stringify(list));
  } catch {}
}

function deleteRecentOwner(name: string): void {
  const list = recentOwners.value.filter((n) => n !== name);
  recentOwners.value = list;
  try {
    localStorage.setItem(RECENT_OWNERS_KEY, JSON.stringify(list));
  } catch {}
}

const filteredRecentOwners = computed(() => {
  const q = (editing.value?.owner_name || "").trim().toLowerCase();
  if (!q) return recentOwners.value;
  return recentOwners.value.filter((name) => name.toLowerCase().includes(q));
});

function selectOwner(name: string): void {
  if (editing.value) {
    editing.value.owner_name = name;
  }
  showOwnerPanel.value = false;
}

function handleOwnerBlur(): void {
  window.setTimeout(() => {
    showOwnerPanel.value = false;
  }, 180);
}

function openCreate(): void {
  if (store.accounts.length === 0) {
    notify("请先在「账号」页新增采集账号并完成登录", "error");
    return;
  }
  editing.value = blank();
  showOwnerPanel.value = false;
  loadRecentOwners();
}

function openEdit(item: MonitoredWork): void {
  editing.value = {
    id: item.id,
    account_id: item.account_id,
    target: item.url || item.aweme_id,
    owner_name: item.owner_name || "",
    enabled: true,
  };
  showOwnerPanel.value = false;
  loadRecentOwners();
}

function handleTargetPaste(e: ClipboardEvent): void {
  const pasted = e.clipboardData?.getData("text") || "";
  const extracted = extractDouyinUrl(pasted);
  if (extracted && extracted !== pasted.trim()) {
    e.preventDefault();
    if (editing.value) {
      editing.value.target = extracted;
    }
    notify("已从分享内容中自动提取视频链接", "info");
  }
}

function cleanTargetInput(): void {
  if (!editing.value?.target) return;
  const extracted = extractDouyinUrl(editing.value.target);
  if (extracted && extracted !== editing.value.target.trim()) {
    editing.value.target = extracted;
  }
}

async function save(): Promise<void> {
  if (!editing.value) return;
  const input = editing.value;
  if (!input.account_id) {
    notify("请选择采集账号", "error");
    return;
  }
  // 提取清洗干净的链接
  input.target = extractDouyinUrl(input.target);
  if (!input.target.trim()) {
    notify("请粘贴视频链接", "error");
    return;
  }
  input.enabled = true;
  setLoading("monitored-save", true);
  try {
    await api.saveMonitored(input);
    if (input.owner_name?.trim()) {
      recordRecentOwner(input.owner_name);
    }
    editing.value = null;
    await Promise.all([refreshMonitored(), refreshStatus()]);
    notify("监控视频已保存", "ok");
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading("monitored-save", false);
  }
}

async function removeMonitored(item: MonitoredWork): Promise<void> {
  if (!window.confirm(`确定不再监控视频「${item.title || item.aweme_id}」？`)) return;
  try {
    await api.deleteMonitored(item.id);
    await Promise.all([refreshMonitored(), refreshStatus()]);
    notify("已移除监控", "ok");
  } catch (err) {
    notify(String(err), "error");
  }
}

async function collectAccount(accountId: number): Promise<void> {
  if (!hasChrome.value) {
    notify("未检测到 Google Chrome 浏览器，无法开始采集。请前往 https://www.google.cn/chrome/ 下载安装", "error");
    void api.openUrl("https://www.google.cn/chrome/");
    return;
  }
  setLoading(`collect-${accountId}`, true);
  try {
    const outcomes = await api.collectNow(accountId);
    const [outcome] = outcomes;
    if (outcome?.ok) {
      notify(outcome.message || "采集成功", "ok");
    } else {
      notify(outcome?.message ?? "采集失败", "error");
    }
    await Promise.all([refreshMonitored(), refreshStatus()]);
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading(`collect-${accountId}`, false);
  }
}

async function collectAll(): Promise<void> {
  if (!hasChrome.value) {
    notify("未检测到 Google Chrome 浏览器，无法执行全量采集。请前往 https://www.google.cn/chrome/ 下载安装", "error");
    void api.openUrl("https://www.google.cn/chrome/");
    return;
  }
  collectAllBusy.value = true;
  try {
    const outcomes = await api.collectAll();
    const successCount = outcomes.filter((o) => o.ok).length;
    notify(`一键采集完成：${successCount}/${outcomes.length} 个账号采集成功`, successCount > 0 ? "ok" : "warn");
    await Promise.all([refreshMonitored(), refreshStatus()]);
  } catch (err) {
    notify(`全量采集失败：${String(err)}`, "error");
  } finally {
    collectAllBusy.value = false;
  }
}

type TimeRange = "24h" | "3d" | "7d";
const selectedRange = ref<TimeRange>("7d");

const TIME_RANGE_SECS: Record<TimeRange, number> = {
  "24h": 24 * 3600,
  "3d": 3 * 86400,
  "7d": 7 * 86400,
};

const TIME_RANGE_LABELS: Record<TimeRange, string> = {
  "24h": "近 24 小时",
  "3d": "近 3 天",
  "7d": "近 7 天",
};

async function openDetail(title: string, workId: number | null): Promise<void> {
  if (!workId) {
    notify("该视频还没有采集到快照数据，请先点击「采集」", "error");
    return;
  }
  detailBusy.value = true;
  try {
    const trend = await api.workTrend(workId, 2500);
    detail.value = { title, workId, trend, metric: "all" };
    selectedRange.value = "7d";
  } catch (err) {
    notify(String(err), "error");
  } finally {
    detailBusy.value = false;
  }
}

interface MetricConfig {
  key: "like" | "comment" | "collect" | "share";
  label: string;
  icon: string;
  color: string;
  fillColor: string;
  badgeClass: string;
}

const METRIC_CONFIGS: Record<string, MetricConfig> = {
  like: {
    key: "like",
    label: "点赞",
    icon: "👍",
    color: "#f43f5e",
    fillColor: "rgba(244, 63, 94, 0.16)",
    badgeClass: "text-rose-400 bg-rose-500/10 border-rose-500/20",
  },
  comment: {
    key: "comment",
    label: "评论",
    icon: "💬",
    color: "#38bdf8",
    fillColor: "rgba(56, 189, 248, 0.16)",
    badgeClass: "text-sky-400 bg-sky-500/10 border-sky-500/20",
  },
  collect: {
    key: "collect",
    label: "收藏",
    icon: "⭐",
    color: "#f59e0b",
    fillColor: "rgba(245, 158, 11, 0.16)",
    badgeClass: "text-amber-400 bg-amber-500/10 border-amber-500/20",
  },
  share: {
    key: "share",
    label: "分享",
    icon: "🔄",
    color: "#a855f7",
    fillColor: "rgba(168, 85, 247, 0.16)",
    badgeClass: "text-purple-400 bg-purple-500/10 border-purple-500/20",
  },
};

interface MetricSummary {
  first: number | null;
  last: number | null;
  delta: number | null;
  max: number | null;
  points: number;
}

const activePoints = computed<SnapshotPoint[]>(() => {
  if (!detail.value || detail.value.trend.length === 0) return [];
  const trend = detail.value.trend;
  const rangeSecs = TIME_RANGE_SECS[selectedRange.value];
  const now = Math.floor(Date.now() / 1000);
  const latestTs = trend[trend.length - 1]?.ts ?? now;
  // 基准时间取当前时间与最后快照时间的较小有效锚点
  const anchorTs = Math.max(now, latestTs);
  const cutoff = anchorTs - rangeSecs;
  const filtered = trend.filter((p) => p.ts >= cutoff);
  // 若当前时间范围筛选后点数不足，且存在历史点，则以最后采集点为基准回退筛选
  if (filtered.length < 2 && trend.length >= 2) {
    const fallbackFiltered = trend.filter((p) => p.ts >= latestTs - rangeSecs);
    return fallbackFiltered.length > 0 ? fallbackFiltered : trend;
  }
  return filtered.length > 0 ? filtered : trend;
});

function valuesOf(metric: string): Array<number | null> {
  return activePoints.value.map((point) => point[metric as keyof SnapshotPoint] as number | null);
}

function metricSummaryOf(metric: string): MetricSummary {
  const vals = valuesOf(metric).filter((v): v is number => v !== null);
  if (vals.length === 0) {
    return { first: null, last: null, delta: null, max: null, points: 0 };
  }
  const first = vals[0];
  const last = vals[vals.length - 1];
  const max = Math.max(...vals);
  return { first, last, delta: last - first, max, points: vals.length };
}

const allSummaries = computed<Record<string, MetricSummary>>(() => {
  if (!detail.value) return {};
  const res: Record<string, MetricSummary> = {};
  for (const k of metricKeys) {
    res[k] = metricSummaryOf(k);
  }
  return res;
});

const summary = computed(() => {
  if (!detail.value || detail.value.metric === "all") return null;
  return allSummaries.value[detail.value.metric] ?? null;
});

function trendRows(): SnapshotPoint[] {
  return [...activePoints.value].reverse();
}

function openLink(url: string): void {
  if (!url) return;
  void api.openUrl(url).catch((err) => notify(String(err), "error"));
}

onMounted(() => {
  void refreshMonitored();
});
</script>

<template>
  <div class="h-full flex flex-col min-h-0 space-y-3">
    <!-- 头部操作区 -->
    <div class="flex items-end justify-between gap-4 pb-1 shrink-0">
      <div>
        <h1 class="text-xl font-bold tracking-tight text-white">监控作品</h1>
        <p class="text-xs text-muted-foreground mt-1">
          精确监控目标视频，实时追踪点赞、评论、分享与收藏指标增量。
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          :disabled="collectAllBusy || !hasChrome || store.monitored.length === 0"
          :title="!hasChrome ? '缺少 Chrome 浏览器，请前往 https://www.google.cn/chrome/ 下载安装' : '触发所有启用的采集账号对名单视频执行采集'"
          @click="collectAll"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': collectAllBusy }" />
          <span>{{ collectAllBusy ? "全量采集中..." : "一键采集全部" }}</span>
        </Button>
        <Button size="sm" @click="openCreate">
          <Plus class="w-3.5 h-3.5" />
          <span>新增监控视频</span>
        </Button>
      </div>
    </div>

    <!-- 搜索与筛选工具栏 -->
    <div class="flex items-center gap-3 flex-wrap shrink-0">
      <div class="relative flex-1 min-w-[240px] max-w-sm">
        <Search class="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-muted-foreground" />
        <Input
          v-model="searchQuery"
          type="text"
          placeholder="搜索视频标题、作者或负责人..."
          class="pl-8 h-8"
        />
        <button
          v-if="searchQuery"
          class="absolute right-2.5 top-2 text-[10px] text-muted-foreground hover:text-white"
          @click="searchQuery = ''"
        >
          清空
        </button>
      </div>

      <!-- 美化后的下拉状态筛选 -->
      <div class="w-[140px]">
        <Select v-model="filterStatus">
          <SelectTrigger>
            <SelectValue placeholder="全部状态">
              {{ filterStatusLabel }}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">全部状态 ({{ store.monitored.length }})</SelectItem>
            <SelectItem value="enabled">监控中 ({{ store.monitored.filter((m) => m.enabled).length }})</SelectItem>
            <SelectItem value="disabled">已停用 ({{ store.monitored.filter((m) => !m.enabled).length }})</SelectItem>
            <SelectItem value="error">存在报错 ({{ store.monitored.filter((m) => !!m.last_error).length }})</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <span class="ml-auto text-xs text-muted-foreground font-mono">
        共 {{ filteredMonitored.length }} 个视频
      </span>
    </div>

    <!-- 监控作品数据表格 (去掉播放量列，支持指标排序，表格内容内部滚动) -->
    <Table containerClass="flex-1 min-h-0 overflow-auto">
      <TableHeader>
        <TableRow>
          <TableHead class="min-w-[200px]">视频标题</TableHead>
          <TableHead class="whitespace-nowrap">作者</TableHead>
          <TableHead class="whitespace-nowrap">负责人</TableHead>
          <TableHead class="whitespace-nowrap">采集账号</TableHead>
          <TableHead class="text-right">
            <button
              type="button"
              class="inline-flex items-center justify-end gap-1 w-full text-xs font-medium hover:text-white transition-colors cursor-pointer select-none"
              :class="{ 'text-white font-semibold': sortField === 'like' }"
              title="点击按点赞数排序"
              @click="toggleSort('like')"
            >
              <span>点赞</span>
              <ArrowUp v-if="sortField === 'like' && sortOrder === 'asc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowDown v-else-if="sortField === 'like' && sortOrder === 'desc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowUpDown v-else class="w-3 h-3 text-muted-foreground/50 hover:text-white" />
            </button>
          </TableHead>
          <TableHead class="text-right">
            <button
              type="button"
              class="inline-flex items-center justify-end gap-1 w-full text-xs font-medium hover:text-white transition-colors cursor-pointer select-none"
              :class="{ 'text-white font-semibold': sortField === 'comment' }"
              title="点击按评论数排序"
              @click="toggleSort('comment')"
            >
              <span>评论</span>
              <ArrowUp v-if="sortField === 'comment' && sortOrder === 'asc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowDown v-else-if="sortField === 'comment' && sortOrder === 'desc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowUpDown v-else class="w-3 h-3 text-muted-foreground/50 hover:text-white" />
            </button>
          </TableHead>
          <TableHead class="text-right">
            <button
              type="button"
              class="inline-flex items-center justify-end gap-1 w-full text-xs font-medium hover:text-white transition-colors cursor-pointer select-none"
              :class="{ 'text-white font-semibold': sortField === 'share' }"
              title="点击按分享数排序"
              @click="toggleSort('share')"
            >
              <span>分享</span>
              <ArrowUp v-if="sortField === 'share' && sortOrder === 'asc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowDown v-else-if="sortField === 'share' && sortOrder === 'desc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowUpDown v-else class="w-3 h-3 text-muted-foreground/50 hover:text-white" />
            </button>
          </TableHead>
          <TableHead class="text-right">
            <button
              type="button"
              class="inline-flex items-center justify-end gap-1 w-full text-xs font-medium hover:text-white transition-colors cursor-pointer select-none"
              :class="{ 'text-white font-semibold': sortField === 'collect' }"
              title="点击按收藏数排序"
              @click="toggleSort('collect')"
            >
              <span>收藏</span>
              <ArrowUp v-if="sortField === 'collect' && sortOrder === 'asc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowDown v-else-if="sortField === 'collect' && sortOrder === 'desc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowUpDown v-else class="w-3 h-3 text-muted-foreground/50 hover:text-white" />
            </button>
          </TableHead>
          <TableHead class="whitespace-nowrap">
            <button
              type="button"
              class="inline-flex items-center gap-1 text-xs font-medium hover:text-white transition-colors cursor-pointer select-none"
              :class="{ 'text-white font-semibold': sortField === 'published_at' }"
              title="点击按发布时间排序"
              @click="toggleSort('published_at')"
            >
              <span>发布时间</span>
              <ArrowUp v-if="sortField === 'published_at' && sortOrder === 'asc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowDown v-else-if="sortField === 'published_at' && sortOrder === 'desc'" class="w-3.5 h-3.5 text-blue-400" />
              <ArrowUpDown v-else class="w-3 h-3 text-muted-foreground/50 hover:text-white" />
            </button>
          </TableHead>
          <TableHead class="whitespace-nowrap">更新时间</TableHead>
          <TableHead class="sticky right-0 top-0 z-30 bg-slate-950 text-center whitespace-nowrap border-l border-slate-800/80 border-b border-slate-800/70 shadow-[-4px_0_8px_-2px_rgba(0,0,0,0.5)] min-w-[220px] px-3">
            操作
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="item in paginatedMonitored" :key="item.id" class="group">
          <!-- 视频标题 -->
          <TableCell class="min-w-[200px] max-w-[280px]">
            <div class="flex items-center gap-2">
              <span
                class="w-1.5 h-1.5 rounded-full shrink-0"
                :class="item.enabled ? 'bg-emerald-400' : 'bg-slate-500'"
              />
              <button
                class="font-medium text-white truncate text-left hover:text-blue-400 hover:underline cursor-pointer"
                :title="item.title || item.aweme_id"
                @click="openDetail(item.title || item.aweme_id, item.work_id)"
              >
                {{ item.title || item.aweme_id }}
              </button>
            </div>
            <div class="text-[11px] text-muted-foreground font-mono mt-0.5 pl-3.5">
              ID {{ item.aweme_id }}{{ item.enabled ? "" : " · 已停用" }}
            </div>
            <div v-if="item.last_error" class="text-[11px] text-amber-400 font-mono mt-0.5 pl-3.5">
              {{ item.last_error }}
            </div>
          </TableCell>

          <!-- 作者 -->
          <TableCell class="text-slate-300 whitespace-nowrap">
            {{ item.author_name || "—" }}
          </TableCell>

          <!-- 负责人 -->
          <TableCell class="whitespace-nowrap">
            <Badge v-if="item.owner_name" variant="info" class="text-[11px] px-2 py-0">
              {{ item.owner_name }}
            </Badge>
            <span v-else class="text-muted-foreground text-xs">—</span>
          </TableCell>

          <!-- 采集账号 -->
          <TableCell class="text-muted-foreground text-xs whitespace-nowrap">
            {{ item.account_name }}
          </TableCell>

          <!-- 指标数据 -->
          <TableCell class="text-right font-mono font-semibold text-white whitespace-nowrap">
            {{ fmtNum(item.metrics.like) }}
          </TableCell>
          <TableCell class="text-right font-mono text-slate-300 whitespace-nowrap">
            {{ fmtNum(item.metrics.comment) }}
          </TableCell>
          <TableCell class="text-right font-mono text-slate-300 whitespace-nowrap">
            {{ fmtNum(item.metrics.share) }}
          </TableCell>
          <TableCell class="text-right font-mono text-slate-300 whitespace-nowrap">
            {{ fmtNum(item.metrics.collect) }}
          </TableCell>

          <!-- 发布时间 -->
          <TableCell
            class="whitespace-nowrap text-muted-foreground font-mono text-[11px]"
            :title="item.published_at ? fmtTime(item.published_at) : '尚未采集到发布时间'"
          >
            {{ fmtPublishDate(item.published_at) }}
          </TableCell>

          <!-- 更新时间 -->
          <TableCell class="whitespace-nowrap text-muted-foreground font-mono text-[11px]">
            {{ fmtShort(item.last_seen_at) }}
          </TableCell>

          <!-- 操作按钮组 (固定右侧，纯文字按钮) -->
          <TableCell class="sticky right-0 z-10 bg-slate-900 group-hover:bg-[#151e31] transition-colors border-l border-slate-800/80 border-b border-slate-800/70 shadow-[-4px_0_8px_-2px_rgba(0,0,0,0.5)] min-w-[220px] p-2 text-center">
            <div class="flex items-center justify-center gap-1">
              <Button
                size="sm"
                variant="ghost"
                class="h-6 px-1.5 text-xs text-blue-400 hover:text-blue-300 hover:bg-blue-500/10 font-normal"
                :disabled="store.loading[`collect-${item.account_id}`] || !hasChrome"
                :title="!hasChrome ? '缺少 Chrome 浏览器，请前往 https://www.google.cn/chrome/ 下载安装' : '立即通过关联账号采集此视频'"
                @click="collectAccount(item.account_id)"
              >
                <RefreshCw v-if="store.loading[`collect-${item.account_id}`]" class="w-3 h-3 animate-spin mr-0.5 inline" />
                <span>{{ store.loading[`collect-${item.account_id}`] ? "采集中" : "采集" }}</span>
              </Button>

              <Button
                size="sm"
                variant="ghost"
                class="h-6 px-1.5 text-xs text-slate-300 hover:text-white hover:bg-slate-800/80 font-normal"
                title="查看数据快照趋势图"
                @click="openDetail(item.title || item.aweme_id, item.work_id)"
              >
                趋势
              </Button>

              <Button
                v-if="item.url || item.aweme_id"
                size="sm"
                variant="ghost"
                class="h-6 px-1.5 text-xs text-slate-300 hover:text-white hover:bg-slate-800/80 font-normal"
                title="在浏览器打开视频页面"
                @click="openLink(item.url || `https://www.douyin.com/video/${item.aweme_id}`)"
              >
                打开
              </Button>

              <Button
                size="sm"
                variant="ghost"
                class="h-6 px-1.5 text-xs text-slate-300 hover:text-white hover:bg-slate-800/80 font-normal"
                title="编辑监控设置"
                @click="openEdit(item)"
              >
                编辑
              </Button>

              <Button
                size="sm"
                variant="ghost"
                class="h-6 px-1.5 text-xs text-rose-400 hover:text-rose-300 hover:bg-rose-500/15 font-normal"
                title="移除监控"
                @click="removeMonitored(item)"
              >
                删除
              </Button>
            </div>
          </TableCell>
        </TableRow>

        <TableEmpty v-if="filteredMonitored.length === 0" :colspan="11">
          <template v-if="store.monitored.length === 0">
            还没有监控视频。点击右上角「新增监控视频」，粘贴抖音作品链接开始数据监控。
          </template>
          <template v-else>
            未找到与当前筛选条件匹配的视频。
          </template>
        </TableEmpty>
      </TableBody>
    </Table>

    <!-- 分页导航条（固定在底部，无需页面滚动） -->
    <Pagination
      v-if="filteredMonitored.length > 0"
      v-model:current-page="currentPage"
      v-model:page-size="pageSize"
      :total-items="filteredMonitored.length"
      class="shrink-0 pt-1 border-t border-border/40"
    />

    <!-- 新增 / 编辑监控视频 Dialog (右上角固定关闭按钮) -->
    <Dialog :open="!!editing" @update:open="(val) => { if (!val) editing = null; }">
      <DialogContent class="sm:max-w-md max-h-[90vh] flex flex-col p-0 overflow-hidden">
        <DialogHeader class="p-6 pb-3 pr-14 border-b border-border/40 shrink-0">
          <DialogTitle>{{ editing?.id === null ? "新增监控视频" : `编辑监控视频 #${editing?.id}` }}</DialogTitle>
          <DialogDescription>
            输入抖音视频链接，选择执行采集的账号会话开始数据追踪。
          </DialogDescription>
        </DialogHeader>

        <div v-if="editing" class="flex-1 overflow-y-auto p-6 space-y-3.5">
          <div class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">采集账号（提供会话环境）</label>
            <Select
              :model-value="String(editing.account_id)"
              @update:model-value="editing.account_id = Number($event)"
            >
              <SelectTrigger>
                <SelectValue placeholder="选择采集账号">
                  {{ currentEditingAccountName }}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="account in store.accounts"
                  :key="account.id"
                  :value="String(account.id)"
                >
                  {{ account.name }} ({{ account.login_state === "ok" ? "已登录" : "未登录" }})
                </SelectItem>
              </SelectContent>
            </Select>
            <p class="text-[11px] text-muted-foreground">使用该账号的独立浏览器会话访问视频页面，支持监控任何公开视频。</p>
          </div>

          <div class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">视频链接</label>
            <Input
              v-model="editing.target"
              spellcheck="false"
              placeholder="粘贴链接或App分享完整文案（自动截取链接）"
              @paste="handleTargetPaste"
              @blur="cleanTargetInput"
            />
            <p class="text-[11px] text-muted-foreground">支持抖音长链、App 分享短链（直接粘贴完整分享文案可自动截取链接）。</p>
          </div>

          <div class="space-y-1.5 relative">
            <div class="flex items-center justify-between">
              <label class="text-xs font-medium text-slate-300">作品负责人（可选）</label>
              <span v-if="filteredRecentOwners.length > 0" class="text-[10px] text-muted-foreground">
                可从最近保存中直接点选
              </span>
            </div>
            <div class="relative">
              <Input
                v-model="editing.owner_name"
                placeholder="例如：张三"
                autocomplete="off"
                @focus="showOwnerPanel = true"
                @blur="handleOwnerBlur"
              />
              <!-- 聚焦下拉浮层面板 -->
              <div
                v-if="showOwnerPanel && filteredRecentOwners.length > 0"
                class="absolute left-0 right-0 top-full mt-1.5 z-50 rounded-md border border-border bg-slate-900/95 backdrop-blur-md p-1 shadow-2xl animate-in fade-in-0 zoom-in-95 duration-100"
              >
                <div class="px-2 py-1 text-[10px] font-medium text-muted-foreground flex items-center justify-between border-b border-border/50 pb-1 mb-1">
                  <span>最近保存的负责人（点击快速填入）</span>
                  <span>{{ filteredRecentOwners.length }} 位</span>
                </div>
                <div class="space-y-0.5 max-h-48 overflow-y-auto">
                  <div
                    v-for="name in filteredRecentOwners"
                    :key="name"
                    class="group flex items-center justify-between px-2.5 py-1.5 text-xs text-slate-200 rounded cursor-pointer transition-colors hover:bg-slate-800 hover:text-white"
                    :class="{ 'bg-slate-800/80 text-white font-medium': editing.owner_name === name }"
                    @mousedown.prevent="selectOwner(name)"
                  >
                    <div class="flex items-center gap-1.5 min-w-0">
                      <User class="w-3 h-3 text-muted-foreground shrink-0" />
                      <span class="truncate">{{ name }}</span>
                    </div>
                    <div class="flex items-center gap-1.5 shrink-0">
                      <Badge v-if="editing.owner_name === name" variant="secondary" class="text-[9px] h-4 px-1 py-0">已选</Badge>
                      <button
                        type="button"
                        class="p-0.5 rounded text-muted-foreground hover:text-red-400 hover:bg-red-500/10 transition-colors"
                        title="从快捷列表中删除该负责人"
                        @mousedown.stop.prevent="deleteRecentOwner(name)"
                      >
                        <X class="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            <p class="text-[11px] text-muted-foreground">指定该视频的责任人，报警消息将带上负责人称呼。</p>
          </div>
        </div>

        <DialogFooter class="p-4 px-6 border-t border-border/40 shrink-0">
          <Button variant="ghost" size="sm" @click="editing = null">取消</Button>
          <Button
            size="sm"
            :disabled="store.loading['monitored-save']"
            @click="save"
          >
            保存监控
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 趋势图 Dialog：支持全部指标同时展示多个折线图，亦可单选聚焦放大，右上角固定关闭按钮 -->
    <Dialog :open="!!detail" @update:open="(val) => { if (!val) detail = null; }">
      <DialogContent class="sm:max-w-4xl w-[96vw] max-w-4xl max-h-[90vh] p-0 flex flex-col overflow-hidden">
        <DialogHeader class="p-6 pb-3 pr-14 border-b border-border/40 shrink-0">
          <DialogTitle class="truncate max-w-lg sm:max-w-xl text-lg font-semibold">{{ detail?.title }}</DialogTitle>
          <DialogDescription>
            视频多维历史数据快照趋势与增长分析（优先记录官方接口精准数值）。
          </DialogDescription>
        </DialogHeader>

        <div v-if="detail" class="flex-1 overflow-y-auto p-4 sm:p-6 space-y-4 min-w-0 max-w-full">
          <!-- 顶部工具栏：时间跨度切换 (24h/3d/7d) 与指标视图切换 (多图并列/聚焦) -->
          <div class="flex flex-wrap items-center justify-between gap-3 border-b border-border/40 pb-3">
            <div class="flex flex-wrap items-center gap-2">
              <!-- 时间区间药丸切换器（默认 7 天） -->
              <div class="flex items-center gap-1 bg-muted/40 p-1 rounded-lg border border-border/50 text-xs">
                <button
                  v-for="range in (['24h', '3d', '7d'] as const)"
                  :key="range"
                  type="button"
                  class="px-2.5 py-1 rounded-md font-medium transition-all"
                  :class="selectedRange === range ? 'bg-primary text-primary-foreground shadow-sm' : 'text-muted-foreground hover:text-white'"
                  @click="selectedRange = range"
                >
                  {{ range === '24h' ? '最近 24 小时' : range === '3d' ? '最近 3 天' : '最近 7 天 (默认)' }}
                </button>
              </div>

              <!-- 指标视图切换器 -->
              <div class="flex items-center gap-1 bg-muted/40 p-1 rounded-lg border border-border/50 text-xs">
                <button
                  type="button"
                  class="px-2.5 py-1 rounded-md font-medium transition-all"
                  :class="detail.metric === 'all' ? 'bg-primary text-primary-foreground shadow-sm' : 'text-muted-foreground hover:text-white'"
                  @click="detail.metric = 'all'"
                >
                  多图并列
                </button>
                <button
                  v-for="k in metricKeys"
                  :key="k"
                  type="button"
                  class="px-2 py-1 rounded-md font-medium transition-all flex items-center gap-1"
                  :class="detail.metric === k ? 'bg-primary text-primary-foreground shadow-sm' : 'text-muted-foreground hover:text-white'"
                  @click="detail.metric = k"
                >
                  <span>{{ METRIC_CONFIGS[k]?.icon }}</span>
                  <span>{{ METRIC_CONFIGS[k]?.label }}</span>
                </button>
              </div>
            </div>

            <div class="text-xs text-muted-foreground font-mono flex items-center gap-2">
              <span>{{ TIME_RANGE_LABELS[selectedRange] }}快照 {{ activePoints.length }} 条</span>
              <span class="text-border">|</span>
              <span>历史总计 {{ detail.trend.length }} 条</span>
            </div>
          </div>

          <!-- 模式 1：多图并列同时展示多个折线图（2x2 网格） -->
          <div v-if="detail.metric === 'all'" class="grid grid-cols-1 sm:grid-cols-2 gap-3.5 w-full">
            <Card
              v-for="k in metricKeys"
              :key="k"
              class="p-3.5 bg-muted/20 border-border/60 hover:border-border transition-colors flex flex-col justify-between"
            >
              <div class="flex items-start justify-between">
                <div>
                  <div class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                    <span class="text-sm">{{ METRIC_CONFIGS[k].icon }}</span>
                    <span>{{ METRIC_CONFIGS[k].label }}趋势</span>
                  </div>
                  <div class="text-xl font-bold font-mono text-white mt-1">
                    {{ fmtNum(allSummaries[k]?.last) }}
                  </div>
                </div>
                <div class="text-right">
                  <div
                    class="text-xs font-mono font-semibold px-2 py-0.5 rounded-full border inline-block"
                    :class="METRIC_CONFIGS[k].badgeClass"
                  >
                    增量 {{ fmtDelta(allSummaries[k]?.delta) }}
                  </div>
                  <div class="text-[11px] text-muted-foreground/80 mt-1 font-mono">
                    峰值 {{ fmtNum(allSummaries[k]?.max) }}
                  </div>
                </div>
              </div>

              <div class="mt-3 pt-2 border-t border-border/30">
                <Sparkline
                  :values="valuesOf(k)"
                  :width="360"
                  :height="90"
                  :color="METRIC_CONFIGS[k].color"
                  :fill-color="METRIC_CONFIGS[k].fillColor"
                  :label="METRIC_CONFIGS[k].label"
                />
              </div>
            </Card>
          </div>

          <!-- 模式 2：聚焦单个大图模式 -->
          <div v-else class="space-y-3.5">
            <div class="grid grid-cols-2 sm:grid-cols-4 gap-2.5 w-full min-w-0">
              <Card class="p-3 bg-muted/20 border-border/50">
                <div class="text-[11px] text-muted-foreground">最新数值</div>
                <div class="text-lg font-bold font-mono text-white mt-0.5">{{ fmtNum(summary?.last ?? null) }}</div>
              </Card>
              <Card class="p-3 bg-muted/20 border-border/50">
                <div class="text-[11px] text-muted-foreground">累计增量</div>
                <div class="text-lg font-bold font-mono text-emerald-400 mt-0.5">{{ fmtDelta(summary?.delta ?? null) }}</div>
              </Card>
              <Card class="p-3 bg-muted/20 border-border/50">
                <div class="text-[11px] text-muted-foreground">历史峰值</div>
                <div class="text-lg font-bold font-mono text-white mt-0.5">{{ fmtNum(summary?.max ?? null) }}</div>
              </Card>
              <Card class="p-3 bg-muted/20 border-border/50">
                <div class="text-[11px] text-muted-foreground">初始快照</div>
                <div class="text-lg font-bold font-mono text-white mt-0.5">{{ fmtNum(summary?.first ?? null) }}</div>
              </Card>
            </div>

            <div class="border border-border/60 rounded-lg p-3 bg-background/80 w-full max-w-full overflow-hidden">
              <Sparkline
                :values="valuesOf(detail.metric)"
                :width="760"
                :height="170"
                :color="METRIC_CONFIGS[detail.metric]?.color"
                :fill-color="METRIC_CONFIGS[detail.metric]?.fillColor"
                :label="metricLabel(detail.metric)"
              />
            </div>
          </div>

          <!-- 底部历史快照明细表（全维度对比表格） -->
          <div class="border border-border/60 rounded-lg overflow-hidden bg-background/60">
            <div class="px-3 py-2 bg-muted/30 border-b border-border/40 text-xs font-semibold text-muted-foreground flex items-center justify-between">
              <span>历史快照明细列表 ({{ TIME_RANGE_LABELS[selectedRange] }}共 {{ trendRows().length }} 条快照)</span>
              <span class="text-[11px] font-normal text-muted-foreground/80">各快照点完整呈现全部指标</span>
            </div>
            <div class="max-h-64 sm:max-h-72 overflow-auto w-full max-w-full">
              <Table>
                <TableHeader class="sticky top-0 bg-slate-950 z-10">
                  <TableRow class="hover:bg-transparent">
                    <TableHead class="whitespace-nowrap">快照时间</TableHead>
                    <TableHead class="text-right whitespace-nowrap">👍 点赞</TableHead>
                    <TableHead class="text-right whitespace-nowrap">💬 评论</TableHead>
                    <TableHead class="text-right whitespace-nowrap">⭐ 收藏</TableHead>
                    <TableHead class="text-right whitespace-nowrap">🔄 分享</TableHead>
                    <TableHead class="text-center whitespace-nowrap">数据来源</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-for="row in trendRows()" :key="row.ts">
                    <TableCell class="font-mono text-muted-foreground text-xs whitespace-nowrap">
                      {{ fmtTime(row.ts) }}
                    </TableCell>
                    <TableCell class="text-right font-mono font-medium text-white whitespace-nowrap">
                      {{ fmtNum(row.like) }}
                    </TableCell>
                    <TableCell class="text-right font-mono font-medium text-white whitespace-nowrap">
                      {{ fmtNum(row.comment) }}
                    </TableCell>
                    <TableCell class="text-right font-mono font-medium text-white whitespace-nowrap">
                      {{ fmtNum(row.collect) }}
                    </TableCell>
                    <TableCell class="text-right font-mono font-medium text-white whitespace-nowrap">
                      {{ fmtNum(row.share) }}
                    </TableCell>
                    <TableCell class="text-center whitespace-nowrap">
                      <span
                        v-if="row.is_approximate"
                        class="text-[10px] text-amber-400 bg-amber-500/10 border border-amber-500/20 px-1.5 py-0.5 rounded font-mono"
                      >
                        预估
                      </span>
                      <span
                        v-else
                        class="text-[10px] text-emerald-400 bg-emerald-500/10 border border-emerald-500/20 px-1.5 py-0.5 rounded font-mono"
                      >
                        接口精准
                      </span>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </div>
          </div>
        </div>

        <DialogFooter class="p-4 px-6 border-t border-border/40 shrink-0">
          <Button variant="ghost" size="sm" @click="detail = null">关闭</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
