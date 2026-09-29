<script setup lang="ts">
import { computed } from "vue";
import { api } from "@/api";
import { notify, store } from "@/store";
import { fmtAgo, fmtDelta, fmtFuture, fmtNum, fmtTime, metricLabel } from "@/utils";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
  Activity,
  AlertTriangle,
  Bell,
  BookOpen,
  CheckCircle2,
  Chrome,
  ExternalLink,
  Pause,
  Play,
  Users,
  Video,
} from "lucide-vue-next";

const CHROME_DOWNLOAD_URL = "https://www.google.cn/chrome/";

async function openChromeDownload(): Promise<void> {
  try {
    await api.openUrl(CHROME_DOWNLOAD_URL);
  } catch (err) {
    notify(`打开下载链接失败：${String(err)}`, "error");
  }
}

const emit = defineEmits<{
  (e: "goto", tab: "accounts" | "works" | "rules" | "alerts" | "settings" | "logs" | "guide"): void;
  (e: "toggle-pause"): void;
}>();

const status = computed(() => store.status);
const accounts = computed(() => store.accounts);
const recentAlerts = computed(() => store.alerts.slice(0, 6));
const hasChrome = computed(() => !!store.status?.chrome_path);

const problemAccounts = computed(() =>
  accounts.value.filter((a) => a.login_state !== "ok" || a.last_error.trim() !== ""),
);

const accountsWithoutMonitor = computed(() => {
  const covered = new Set(store.monitored.map((item) => item.account_id));
  return accounts.value.filter((a) => !covered.has(a.id));
});

const collecting = computed(() => store.status?.collecting ?? false);
const paused = computed(() => store.status?.paused ?? false);

const autoHint = computed(() => {
  const next = status.value?.next_run_at ?? null;
  if (!next) return "自动采集调度未启动";
  return `下次计划调度约 ${fmtFuture(next)}`;
});
</script>

<template>
  <div class="h-full overflow-y-auto space-y-4 pr-1">
    <!-- 页面标题与快捷动作 -->
    <div class="flex items-end justify-between gap-4 pb-1">
      <div>
        <h1 class="text-xl font-bold tracking-tight text-white">系统概览</h1>
        <p class="text-xs text-muted-foreground mt-1">
          {{ autoHint }} · 调度周期 10 分钟 · 仅对「监控作品」名单视频实施数据追踪
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          class="gap-1.5 text-xs text-sky-400 border-sky-500/30 hover:bg-sky-500/10"
          @click="emit('goto', 'guide')"
        >
          <BookOpen class="w-3.5 h-3.5" />
          <span>使用教程</span>
        </Button>
        <Button
          variant="outline"
          size="sm"
          class="gap-1.5"
          @click="emit('toggle-pause')"
        >
          <component :is="paused ? Play : Pause" class="w-3.5 h-3.5" />
          <span>{{ paused ? "恢复自动采集" : "暂停自动采集" }}</span>
        </Button>
      </div>
    </div>

    <!-- 顶部四项指标看板 (shadcn Card) -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
      <!-- 采集账号 -->
      <Card class="bg-card/70 border-border/70 hover:border-slate-600 transition-colors">
        <CardHeader class="flex flex-row items-center justify-between pb-2">
          <span class="text-xs font-medium text-muted-foreground">采集账号</span>
          <Users class="w-4 h-4 text-blue-400" />
        </CardHeader>
        <CardContent>
          <div class="text-2xl font-bold tracking-tight text-white font-mono">
            {{ status?.accounts_ok ?? 0 }}
            <span class="text-sm font-normal text-muted-foreground"> / {{ status?.accounts_total ?? 0 }}</span>
          </div>
          <p class="text-[11px] text-muted-foreground mt-1">有效登录 / 账号总数</p>
        </CardContent>
      </Card>

      <!-- 监控视频 -->
      <Card class="bg-card/70 border-border/70 hover:border-slate-600 transition-colors">
        <CardHeader class="flex flex-row items-center justify-between pb-2">
          <span class="text-xs font-medium text-muted-foreground">监控视频</span>
          <Video class="w-4 h-4 text-emerald-400" />
        </CardHeader>
        <CardContent>
          <div class="text-2xl font-bold tracking-tight text-white font-mono">
            {{ status?.monitored_total ?? 0 }}
          </div>
          <p class="text-[11px] text-muted-foreground mt-1">
            已记录 {{ fmtNum(status?.snapshots_total ?? 0) }} 次快照
          </p>
        </CardContent>
      </Card>

      <!-- 今日报警 -->
      <Card class="bg-card/70 border-border/70 hover:border-slate-600 transition-colors">
        <CardHeader class="flex flex-row items-center justify-between pb-2">
          <span class="text-xs font-medium text-muted-foreground">今日报警</span>
          <Bell class="w-4 h-4 text-amber-400" />
        </CardHeader>
        <CardContent>
          <div class="text-2xl font-bold tracking-tight text-white font-mono">
            {{ status?.alerts_today ?? 0 }}
          </div>
          <p class="text-[11px] text-muted-foreground mt-1">
            最近一次 {{ fmtAgo(store.alerts[0]?.created_at ?? null) }}
          </p>
        </CardContent>
      </Card>

      <!-- 采集状态 -->
      <Card class="bg-card/70 border-border/70 hover:border-slate-600 transition-colors">
        <CardHeader class="flex flex-row items-center justify-between pb-2">
          <span class="text-xs font-medium text-muted-foreground">采集状态</span>
          <Activity class="w-4 h-4 text-sky-400" />
        </CardHeader>
        <CardContent>
          <div class="h-8 flex items-center">
            <div v-if="collecting" class="flex items-center gap-2">
              <span class="pulse" />
              <span class="text-xs font-medium text-amber-300 truncate max-w-[120px]">
                {{ status?.current_account || "采集中" }}
              </span>
            </div>
            <Badge v-else-if="paused" variant="warning">已暂停调度</Badge>
            <Badge v-else variant="success" class="gap-1.5">
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 inline-block" />
              <span>就绪待命</span>
            </Badge>
          </div>
          <p class="text-[11px] text-muted-foreground mt-1">
            上次完成 {{ fmtAgo(status?.last_run_at ?? null) }}
          </p>
        </CardContent>
      </Card>
    </div>

    <!-- 待处理事项卡片 -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader class="flex flex-row items-center justify-between">
        <div class="flex items-center gap-2">
          <AlertTriangle class="w-4 h-4 text-amber-400" />
          <CardTitle>待处理事项</CardTitle>
        </div>
        <Badge :variant="!hasChrome || problemAccounts.length || accountsWithoutMonitor.length ? 'warning' : 'success'">
          {{ (!hasChrome ? 1 : 0) + problemAccounts.length + accountsWithoutMonitor.length }} 项关注
        </Badge>
      </CardHeader>
      <CardContent class="pt-0">
        <!-- 未安装 Chrome 提示 -->
        <div
          v-if="!hasChrome"
          class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 p-3 rounded-lg bg-red-950/30 border border-red-500/30 mb-3"
        >
          <div class="flex items-center gap-2.5">
            <Chrome class="w-4 h-4 text-red-400 shrink-0" />
            <span class="text-xs text-red-300 font-medium">
              未检测到 Google Chrome 浏览器：应用依赖 Chrome 执行自动化采集与账号登录。官方下载地址：
              <a
                :href="CHROME_DOWNLOAD_URL"
                class="text-blue-400 underline hover:text-blue-300 cursor-pointer"
                @click.prevent="openChromeDownload"
              >{{ CHROME_DOWNLOAD_URL }}</a>
            </span>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <Button size="sm" variant="destructive" @click="openChromeDownload">
              <ExternalLink class="w-3.5 h-3.5 mr-1" />
              <span>下载 Chrome</span>
            </Button>
            <Button size="sm" variant="outline" class="border-red-500/30 hover:bg-red-500/20 text-slate-200" @click="emit('goto', 'settings')">
              去配置
            </Button>
          </div>
        </div>

        <div
          v-if="hasChrome && problemAccounts.length === 0 && accountsWithoutMonitor.length === 0"
          class="flex items-center gap-2 py-3 text-xs text-muted-foreground"
        >
          <CheckCircle2 class="w-4 h-4 text-emerald-400" />
          <span>所有运行环境与账号均处于正常状态，暂无需要处理的异常。</span>
        </div>

        <!-- 异常账号列表 -->
        <Table v-else-if="problemAccounts.length || accountsWithoutMonitor.length">
          <TableHeader>
            <TableRow>
              <TableHead>目标对象</TableHead>
              <TableHead>当前状态</TableHead>
              <TableHead>异常说明</TableHead>
              <TableHead class="w-1"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="acc in accountsWithoutMonitor" :key="`m-${acc.id}`">
              <TableCell class="font-medium text-white">{{ acc.name }}</TableCell>
              <TableCell>
                <Badge variant="warning">未配置视频</Badge>
              </TableCell>
              <TableCell class="text-xs text-muted-foreground">尚未配置监控视频，不会触发采集</TableCell>
              <TableCell class="text-right">
                <Button size="sm" variant="default" @click="emit('goto', 'works')">添加视频</Button>
              </TableCell>
            </TableRow>
            <TableRow v-for="acc in problemAccounts" :key="`p-${acc.id}`">
              <TableCell class="font-medium text-white">{{ acc.name }}</TableCell>
              <TableCell>
                <Badge :variant="acc.login_state === 'challenge' ? 'destructive' : 'warning'">
                  {{ acc.login_state === "challenge" ? "需人机验证" : acc.login_state === "expired" ? "登录已失效" : acc.login_state === "unknown" ? "待登录" : "异常" }}
                </Badge>
              </TableCell>
              <TableCell class="text-xs text-muted-foreground truncate max-w-[280px]" :title="acc.last_error">
                {{ acc.last_error || "请打开登录窗口重新扫码确认" }}
              </TableCell>
              <TableCell class="text-right">
                <Button size="sm" variant="secondary" @click="emit('goto', 'accounts')">去登录</Button>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <!-- 最近报警动态卡片 -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader class="flex flex-row items-center justify-between">
        <div class="flex items-center gap-2">
          <Bell class="w-4 h-4 text-sky-400" />
          <CardTitle>最近报警动态</CardTitle>
        </div>
        <Button size="sm" variant="ghost" class="text-xs" @click="emit('goto', 'alerts')">
          查看全部
        </Button>
      </CardHeader>
      <CardContent class="pt-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>触发时间</TableHead>
              <TableHead>规则名称</TableHead>
              <TableHead>作者</TableHead>
              <TableHead>作品标题</TableHead>
              <TableHead>监控指标</TableHead>
              <TableHead class="text-right">增量</TableHead>
              <TableHead>推送状态</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="alert in recentAlerts" :key="alert.id">
              <TableCell class="text-muted-foreground whitespace-nowrap font-mono text-xs">
                {{ fmtTime(alert.created_at) }}
              </TableCell>
              <TableCell class="font-medium text-white">{{ alert.rule_name }}</TableCell>
              <TableCell class="text-muted-foreground">{{ alert.author_name || "—" }}</TableCell>
              <TableCell class="truncate max-w-[240px] text-slate-300" :title="alert.work_title">
                {{ alert.work_title }}
              </TableCell>
              <TableCell>{{ metricLabel(alert.metric) }}</TableCell>
              <TableCell class="text-right font-mono font-semibold text-emerald-400">
                {{ fmtDelta(alert.delta) }}
              </TableCell>
              <TableCell>
                <Badge
                  :variant="alert.notify_state === 'sent' ? 'success' : alert.notify_state === 'failed' ? 'destructive' : 'secondary'"
                >
                  {{ alert.notify_state === "sent" ? "已送达" : alert.notify_state === "failed" ? "失败" : "未发送" }}
                </Badge>
              </TableCell>
            </TableRow>
            <TableEmpty v-if="recentAlerts.length === 0" :colspan="7">
              暂无报警记录。当监控视频数据在指定时间窗口内突破设定的增长阈值时，会在此展示并即时推送飞书通知。
            </TableEmpty>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <!-- 运行依赖卡片 -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader>
        <div class="flex items-center gap-2">
          <Chrome class="w-4 h-4 text-slate-400" />
          <CardTitle>运行依赖状态</CardTitle>
        </div>
      </CardHeader>
      <CardContent class="pt-0">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-xs">
          <div class="flex items-center justify-between p-3 rounded-lg border border-border/60 bg-muted/20">
            <div class="space-y-0.5">
              <div class="font-medium text-white">Google Chrome</div>
              <div class="text-muted-foreground truncate max-w-[260px]" :title="status?.chrome_path">
                {{ status?.chrome_path || "未检测到 Chrome 浏览器" }}
              </div>
            </div>
            <div class="flex items-center gap-2">
              <Badge :variant="hasChrome ? 'success' : 'destructive'">
                {{ hasChrome ? "已就绪" : "未就绪" }}
              </Badge>
              <Button
                v-if="!hasChrome"
                size="sm"
                variant="destructive"
                class="h-7 text-xs"
                @click="openChromeDownload"
              >
                <ExternalLink class="w-3 h-3 mr-1" />
                下载 Chrome
              </Button>
              <Button size="sm" variant="ghost" class="h-7 text-xs" @click="emit('goto', 'settings')">
                {{ hasChrome ? "配置" : "手动指定" }}
              </Button>
            </div>
          </div>

          <div class="flex items-center justify-between p-3 rounded-lg border border-border/60 bg-muted/20">
            <div class="space-y-0.5">
              <div class="font-medium text-white">飞书机器人推送</div>
              <div class="text-muted-foreground">
                {{ status?.feishu_configured ? "Webhook 推送已就绪" : "尚未在系统设置中配置 Webhook" }}
              </div>
            </div>
            <div class="flex items-center gap-2">
              <Badge :variant="status?.feishu_configured && status?.feishu_enabled ? 'success' : 'secondary'">
                {{ status?.feishu_configured && status?.feishu_enabled ? "已启用" : "未启用" }}
              </Badge>
              <Button size="sm" variant="ghost" class="h-7 text-xs" @click="emit('goto', 'settings')">
                前往设置
              </Button>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
