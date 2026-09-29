<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { api } from "@/api";
import { notify, refreshEnvironment, refreshSettings, refreshStatus, setLoading, store } from "@/store";
import type { FeishuConfig, NotifyResult, Settings, StorageStats } from "@/types";
import { fmtBytes, fmtNum, fmtAgo } from "@/utils";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Activity,
  AlertTriangle,
  BellRing,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Chrome,
  Database,
  ExternalLink,
  FileCode,
  FolderOpen,
  Globe,
  HardDrive,
  RefreshCw,
  Send,
  ShieldCheck,
  Sliders,
} from "lucide-vue-next";

const form = reactive<Settings>({
  headless: true,
  page_wait_secs: 8,
  max_works: 50,
  scroll_rounds: 6,
  card_selectors: "",
  metric_labels: "",
  chrome_path: "",
  autostart: null,
  retention_days: 7,
  auto_clean: true,
});

const autostartBusy = ref(false);
const rechecking = ref(false);
const storageStats = ref<StorageStats | null>(null);
const cleaningStorage = ref(false);

const feishuForm = reactive<FeishuConfig>({
  enabled: true,
  webhook: "",
  secret: "",
  keyword: "",
});
const showFeishuAdvanced = ref(false);
const testingFeishu = ref(false);
const feishuTestResult = ref<NotifyResult | null>(null);

const isFeishuConfigured = computed(() => {
  const w = feishuForm.webhook.trim();
  return w.startsWith("https://") && w.length > 20;
});

const hasChrome = computed(() => !!store.status?.chrome_path);

function assign(settings: Settings): void {
  Object.assign(form, settings);
}

async function load(): Promise<void> {
  await refreshSettings();
  if (store.settings) assign(store.settings);
}

async function loadFeishu(): Promise<void> {
  try {
    const cfg = await api.getFeishuConfig();
    Object.assign(feishuForm, cfg);
  } catch {}
}

async function loadStorage(): Promise<void> {
  try {
    storageStats.value = await api.getStorageStats();
  } catch {}
}

async function save(): Promise<void> {
  setLoading("settings-save", true);
  try {
    const [saved] = await Promise.all([
      api.saveSettings({ ...form }),
      api.saveFeishuConfig({ ...feishuForm }),
    ]);
    assign(saved);
    await Promise.all([refreshStatus(), loadStorage()]);
    notify("系统设置与飞书通知配置已保存", "ok");
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading("settings-save", false);
  }
}

async function handleTestFeishu(): Promise<void> {
  if (!feishuForm.webhook.trim()) {
    notify("请先填写飞书群自定义机器人的 Webhook 地址", "warn");
    return;
  }
  testingFeishu.value = true;
  feishuTestResult.value = null;
  try {
    const res = await api.testFeishu(feishuForm);
    feishuTestResult.value = res;
    if (res.ok) {
      notify("飞书测试消息已发送，请检查群内通知！", "ok");
    } else {
      notify(`飞书响应错误 (${res.code}): ${res.msg}`, "error");
    }
  } catch (err) {
    notify(`测试失败：${String(err)}`, "error");
  } finally {
    testingFeishu.value = false;
  }
}

async function checkChrome(): Promise<void> {
  rechecking.value = true;
  try {
    const found = await api.recheckChrome();
    await refreshStatus();
    if (found) {
      notify(`检测到 Chrome 路径：${found}`, "ok");
    } else {
      notify("未在系统或自定义路径中检测到 Google Chrome，请前往 https://www.google.cn/chrome/ 下载安装", "error");
    }
  } catch (err) {
    notify(`检测失败：${String(err)}`, "error");
  } finally {
    rechecking.value = false;
  }
}

async function openChromeDownload(): Promise<void> {
  try {
    await api.openUrl("https://www.google.cn/chrome/");
  } catch (err) {
    notify(`打开下载链接失败：${String(err)}`, "error");
  }
}

async function toggleAutostart(): Promise<void> {
  const next = form.autostart !== true;
  autostartBusy.value = true;
  try {
    const applied = await api.setAutostart(next);
    await load();
    notify(
      applied
        ? "已开启开机自启动：登录系统后在后台托盘静默运行"
        : "已关闭开机自启动",
      "ok",
    );
  } catch (err) {
    notify(String(err), "error");
    await load().catch(() => undefined);
  } finally {
    autostartBusy.value = false;
  }
}

async function handleCleanStorage(): Promise<void> {
  if (!window.confirm(`确定执行数据覆盖清理？\n将清理超过 ${form.retention_days} 天的历史快照与未监控作品，并执行 SQLite 磁盘空间整理。`)) {
    return;
  }
  cleaningStorage.value = true;
  try {
    const res = await api.cleanStorage(form.retention_days);
    notify(
      `覆盖清理完成：已淘汰 ${res.deleted_snapshots} 条超期快照、${res.deleted_works} 个孤立作品，释放磁盘空间 ${fmtBytes(res.freed_bytes)}`,
      "ok",
    );
    await Promise.all([loadStorage(), refreshStatus()]);
  } catch (err) {
    notify(`清理失败：${String(err)}`, "error");
  } finally {
    cleaningStorage.value = false;
  }
}

const envReport = computed(() => store.envReport);
const envChecking = computed(() => store.envChecking);

async function runHealthCheck(): Promise<void> {
  try {
    const rep = await refreshEnvironment();
    await Promise.all([refreshStatus(), loadStorage()]);
    if (rep?.all_passed) {
      notify("全项运行环境体检通过，软件处于最佳工作状态", "ok");
    } else {
      notify(`环境诊断发现隐患：${rep?.summary}`, "warn");
    }
  } catch (err) {
    notify(`体检异常：${String(err)}`, "error");
  }
}

async function openWebviewDownload(): Promise<void> {
  if (envReport.value?.webview2.download_url) {
    void api.openUrl(envReport.value.webview2.download_url);
  }
}

onMounted(() => {
  void Promise.all([load(), loadFeishu(), loadStorage(), refreshEnvironment()]);
});
</script>

<template>
  <div class="h-full overflow-y-auto space-y-4 pr-1">
    <div class="flex items-end justify-between gap-4 pb-1">
      <div>
        <h1 class="text-xl font-bold tracking-tight text-white">系统设置</h1>
        <p class="text-xs text-muted-foreground mt-1">管理系统运行环境、Chrome 浏览器依赖、数据调度与开机自启动选项。</p>
      </div>
      <div class="flex items-center gap-2">
        <Button size="sm" :disabled="store.loading['settings-save']" @click="save">
          <Check class="w-3.5 h-3.5" />
          <span>保存设置</span>
        </Button>
      </div>
    </div>

    <!-- 运行环境体检与诊断中心 (shadcn Card) -->
    <Card class="bg-card/70 border-border/70 overflow-hidden">
      <CardHeader class="flex flex-row items-center justify-between pb-3 bg-muted/10 border-b border-border/30">
        <div class="flex items-center gap-2">
          <Activity class="w-4 h-4 text-emerald-400" />
          <div>
            <CardTitle class="text-sm font-semibold text-white">运行环境全项体检与健康诊断</CardTitle>
            <p class="text-[11px] text-muted-foreground mt-0.5">
              实时监测客户端渲染引擎 (WebView2)、数据采集核心 (Chrome)、网络延迟与本地磁盘健康度
            </p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Badge
            :variant="envReport?.all_passed ? 'success' : 'warning'"
            class="font-mono text-xs px-2.5 py-0.5"
          >
            {{ envReport ? `${envReport.score} 分 · ${envReport.all_passed ? '完美运行' : '需注意'}` : '检测中...' }}
          </Badge>
          <Button
            size="sm"
            variant="outline"
            :disabled="envChecking"
            @click="runHealthCheck"
          >
            <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': envChecking }" />
            <span>{{ envChecking ? "体检中..." : "重新体检" }}</span>
          </Button>
        </div>
      </CardHeader>

      <CardContent class="p-4 space-y-3.5">
        <!-- 综合结论摘要 -->
        <div
          v-if="envReport"
          class="px-3 py-2 rounded-lg text-xs flex items-center gap-2 border"
          :class="
            envReport.all_passed
              ? 'bg-emerald-950/20 border-emerald-500/20 text-emerald-300'
              : 'bg-amber-950/20 border-amber-500/20 text-amber-300'
          "
        >
          <CheckCircle2 v-if="envReport.all_passed" class="w-4 h-4 shrink-0 text-emerald-400" />
          <AlertTriangle v-else class="w-4 h-4 shrink-0 text-amber-400" />
          <span>{{ envReport.summary }}</span>
        </div>

        <!-- 4 维体检卡片网格 -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
          <!-- 1. WebView2 运行时 -->
          <div class="p-3 rounded-lg border border-border/50 bg-muted/20 flex flex-col justify-between">
            <div>
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-200">
                  <ShieldCheck class="w-4 h-4 text-sky-400" />
                  <span>Microsoft Edge WebView2</span>
                </div>
                <Badge :variant="envReport?.webview2.ok ? 'success' : 'destructive'" class="text-[10px]">
                  {{ envReport?.webview2.ok ? '已就绪' : '缺失' }}
                </Badge>
              </div>
              <div class="mt-2 text-xs font-mono text-slate-300">
                版本：{{ envReport?.webview2.version || '未检测到运行时' }}
              </div>
              <p class="text-[11px] text-muted-foreground mt-1 leading-relaxed">
                {{ envReport?.webview2.detail || '检测 WebView2 运行库状态' }}
              </p>
            </div>
            <div v-if="!envReport?.webview2.ok" class="mt-3 pt-2 border-t border-border/30">
              <Button size="sm" variant="destructive" class="h-7 text-xs w-full" @click="openWebviewDownload">
                <ExternalLink class="w-3 h-3 mr-1" />
                <span>下载微软官方 WebView2 运行时</span>
              </Button>
            </div>
          </div>

          <!-- 2. Google Chrome 核心 -->
          <div class="p-3 rounded-lg border border-border/50 bg-muted/20 flex flex-col justify-between">
            <div>
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-200">
                  <Chrome class="w-4 h-4 text-blue-400" />
                  <span>Google Chrome 采集内核</span>
                </div>
                <Badge :variant="envReport?.chrome.ok ? 'success' : 'destructive'" class="text-[10px]">
                  {{ envReport?.chrome.ok ? '已就绪' : '未安装' }}
                </Badge>
              </div>
              <div class="mt-2 text-xs font-mono text-slate-300 truncate" :title="envReport?.chrome.path || ''">
                版本：{{ envReport?.chrome.version || '未知' }}
                <span v-if="envReport?.chrome.path" class="text-muted-foreground text-[10px] ml-1">
                  ({{ envReport.chrome.path }})
                </span>
              </div>
              <p class="text-[11px] text-muted-foreground mt-1 leading-relaxed">
                {{ envReport?.chrome.detail || '检测 Chrome 浏览器及调试端口' }}
              </p>
            </div>
            <div v-if="!envReport?.chrome.ok" class="mt-3 pt-2 border-t border-border/30">
              <Button size="sm" variant="destructive" class="h-7 text-xs w-full" @click="openChromeDownload">
                <ExternalLink class="w-3 h-3 mr-1" />
                <span>前往 Google 官网下载 Chrome</span>
              </Button>
            </div>
          </div>

          <!-- 3. 网络与接口连通性 -->
          <div class="p-3 rounded-lg border border-border/50 bg-muted/20 flex flex-col justify-between">
            <div>
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-200">
                  <Globe class="w-4 h-4 text-emerald-400" />
                  <span>网络与抖音接口连通性</span>
                </div>
                <Badge :variant="envReport?.network.ok ? 'success' : 'warning'" class="text-[10px]">
                  {{ envReport?.network.ok ? '正常' : '连通受阻' }}
                </Badge>
              </div>
              <div class="mt-2 text-xs font-mono text-slate-300 space-y-0.5">
                <div>
                  抖音主站：
                  <span :class="envReport?.network.douyin_main_ok ? 'text-emerald-400' : 'text-red-400'">
                    {{ envReport?.network.douyin_main_ok ? `正常 (${envReport.network.douyin_main_latency_ms}ms)` : '不可达' }}
                  </span>
                </div>
                <div>
                  短链解析：
                  <span :class="envReport?.network.douyin_short_ok ? 'text-emerald-400' : 'text-amber-400'">
                    {{ envReport?.network.douyin_short_ok ? `正常 (${envReport.network.douyin_short_latency_ms}ms)` : '解析异常' }}
                  </span>
                </div>
              </div>
              <p class="text-[11px] text-muted-foreground mt-1 leading-relaxed">
                {{ envReport?.network.detail || '实时检测抖音官方域名网络质量' }}
              </p>
            </div>
          </div>

          <!-- 4. 本地存储与数据库 -->
          <div class="p-3 rounded-lg border border-border/50 bg-muted/20 flex flex-col justify-between">
            <div>
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-1.5 text-xs font-semibold text-slate-200">
                  <HardDrive class="w-4 h-4 text-amber-400" />
                  <span>存储空间与数据库健康</span>
                </div>
                <Badge :variant="envReport?.storage.ok ? 'success' : 'warning'" class="text-[10px]">
                  {{ envReport?.storage.ok ? '健康' : '空间紧张' }}
                </Badge>
              </div>
              <div class="mt-2 text-xs font-mono text-slate-300 space-y-0.5">
                <div>
                  磁盘剩余：
                  <span class="text-white">
                    {{ envReport?.storage.free_bytes ? fmtBytes(envReport.storage.free_bytes) : '--' }}
                  </span>
                  <span class="text-muted-foreground text-[10px] ml-1">
                    (总计 {{ envReport?.storage.total_bytes ? fmtBytes(envReport.storage.total_bytes) : '--' }})
                  </span>
                </div>
                <div>
                  SQLite 库体：
                  <span class="text-white">
                    {{ envReport?.storage.db_size_bytes ? fmtBytes(envReport.storage.db_size_bytes) : '--' }}
                  </span>
                  <span :class="envReport?.storage.db_ok ? 'text-emerald-400 text-[10px] ml-1' : 'text-red-400 text-[10px] ml-1'">
                    {{ envReport?.storage.db_ok ? '(完整性校验通过)' : '(完整性检查异常)' }}
                  </span>
                </div>
              </div>
              <p class="text-[11px] text-muted-foreground mt-1 leading-relaxed">
                {{ envReport?.storage.detail || '检测 AppData 本地数据目录与 SQLite 状态' }}
              </p>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Chrome 运行依赖 (shadcn Card) -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader class="flex flex-row items-center justify-between pb-3">
        <div class="flex items-center gap-2">
          <Chrome class="w-4 h-4 text-blue-400" />
          <CardTitle>Google Chrome 运行环境</CardTitle>
        </div>
        <Badge :variant="hasChrome ? 'success' : 'destructive'">
          {{ hasChrome ? "Chrome 已就绪" : "未检测到 Chrome" }}
        </Badge>
      </CardHeader>
      <CardContent class="space-y-3 pt-0">
        <div class="space-y-1.5">
          <label class="text-xs font-medium text-slate-300">
            Chrome 可执行程序路径（留空则自动扫描标准安装路径）
          </label>
          <div class="flex items-center gap-2">
            <Input
              v-model="form.chrome_path"
              class="flex-1 font-mono"
              spellcheck="false"
              :placeholder="store.status?.chrome_path || '例如：C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'"
            />
            <Button size="sm" variant="outline" :disabled="rechecking" @click="checkChrome">
              <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': rechecking }" />
              <span>{{ rechecking ? "检测中..." : "重新检测" }}</span>
            </Button>
            <Button v-if="!hasChrome" size="sm" variant="destructive" @click="openChromeDownload">
              <ExternalLink class="w-3.5 h-3.5" />
              <span>下载 Chrome</span>
            </Button>
          </div>
          <p class="text-[11px] text-muted-foreground">
            本应用基于浏览器自动化技术进行数据采集，请确保安装了官方正版 Google Chrome。
            <a
              href="https://www.google.cn/chrome/"
              class="text-blue-400 hover:underline cursor-pointer ml-1 inline-flex items-center gap-0.5"
              @click.prevent="openChromeDownload"
            >
              官方下载地址：https://www.google.cn/chrome/
            </a>
            <span v-if="store.status?.chrome_path" class="text-slate-400 block mt-0.5 font-mono">
              当前探测生效路径：{{ store.status.chrome_path }}
            </span>
          </p>
        </div>

        <label class="flex items-center gap-2 cursor-pointer pt-1">
          <input v-model="form.headless" type="checkbox" class="rounded border-border" />
          <span class="text-xs text-slate-200">
            采集时采用无界面后台模式（推荐开启；若遇频繁验证可关闭改用可见窗口排查）
          </span>
        </label>
      </CardContent>
    </Card>

    <!-- 飞书通知机器人配置 (shadcn Card) -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader class="flex flex-row items-center justify-between pb-3">
        <div class="flex items-center gap-2">
          <BellRing class="w-4 h-4 text-sky-400" />
          <div>
            <CardTitle class="text-sm font-semibold text-white">飞书通知机器人配置</CardTitle>
            <p class="text-[11px] text-muted-foreground mt-0.5">
              监控指标达到报警阈值时，自动向飞书群机器人发送告警卡片
            </p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Badge
            :variant="!isFeishuConfigured ? 'secondary' : feishuForm.enabled ? 'success' : 'warning'"
            class="text-xs"
          >
            {{ !isFeishuConfigured ? "未配置 Webhook" : feishuForm.enabled ? "已开启推送" : "已暂停推送" }}
          </Badge>
          <Button
            size="sm"
            variant="outline"
            :disabled="testingFeishu || !feishuForm.webhook.trim()"
            @click="handleTestFeishu"
          >
            <Send class="w-3.5 h-3.5" :class="{ 'animate-pulse': testingFeishu }" />
            <span>{{ testingFeishu ? "测试中..." : "测试连接" }}</span>
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-3.5 pt-0">
        <!-- 默认只显示设置 URL -->
        <div class="space-y-1.5">
          <label class="text-xs font-medium text-slate-300">
            飞书 Webhook 地址 (URL)
          </label>
          <div class="flex items-center gap-2">
            <Input
              v-model="feishuForm.webhook"
              class="flex-1 font-mono text-xs"
              placeholder="https://open.feishu.cn/open-apis/bot/v2/hook/xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
              spellcheck="false"
            />
          </div>
          <p class="text-[11px] text-muted-foreground">
            在飞书群「设置 -> 群机器人 -> 添加机器人 -> 自定义机器人」中添加后复制 Webhook 地址粘贴于此。
          </p>
        </div>

        <!-- 测试结果提示 -->
        <div
          v-if="feishuTestResult"
          class="px-3 py-2 rounded-lg text-xs flex items-center justify-between border"
          :class="
            feishuTestResult.ok
              ? 'bg-emerald-950/20 border-emerald-500/20 text-emerald-300'
              : 'bg-red-950/20 border-red-500/20 text-red-300'
          "
        >
          <div class="flex items-center gap-2">
            <CheckCircle2 v-if="feishuTestResult.ok" class="w-4 h-4 shrink-0 text-emerald-400" />
            <AlertTriangle v-else class="w-4 h-4 shrink-0 text-red-400" />
            <span>{{ feishuTestResult.ok ? "测试消息发送成功，通知通道正常！请前往飞书群确认接收情况。" : `测试发送失败 (错误码 ${feishuTestResult.code})：${feishuTestResult.msg}` }}</span>
          </div>
          <button
            type="button"
            class="text-[11px] opacity-70 hover:opacity-100 ml-2 shrink-0 cursor-pointer"
            @click="feishuTestResult = null"
          >
            关闭
          </button>
        </div>

        <!-- 高级设置折叠开关 -->
        <div class="pt-1">
          <button
            type="button"
            class="flex items-center gap-1.5 text-xs text-slate-400 hover:text-white transition-colors cursor-pointer"
            @click="showFeishuAdvanced = !showFeishuAdvanced"
          >
            <component :is="showFeishuAdvanced ? ChevronUp : ChevronDown" class="w-3.5 h-3.5 text-sky-400" />
            <span>飞书高级设置（推送开关、签名密钥、自定义关键词）</span>
            <Badge variant="outline" class="text-[10px] ml-1 px-1.5 py-0 border-border/60">
              {{ showFeishuAdvanced ? "点击收起" : "点击展开" }}
            </Badge>
          </button>
        </div>

        <!-- 高级设置展开内容 -->
        <div
          v-if="showFeishuAdvanced"
          class="p-3.5 rounded-lg border border-border/50 bg-muted/15 space-y-3.5 animate-in fade-in-0 duration-150"
        >
          <!-- 启用开关 -->
          <label class="flex items-center gap-2 cursor-pointer">
            <input v-model="feishuForm.enabled" type="checkbox" class="rounded border-border" />
            <span class="text-xs text-slate-200">
              启用飞书群机器人自动报警推送（若取消勾选，触发规则时仅记录在本地「通知记录」，不向群发消息）
            </span>
          </label>

          <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-1">
            <!-- 签名密钥 -->
            <div class="space-y-1">
              <label class="text-xs font-medium text-slate-300">
                安全校验：签名密钥 (Secret)
              </label>
              <Input
                v-model="feishuForm.secret"
                class="font-mono text-xs"
                placeholder="未开启“签名校验”请留空"
                spellcheck="false"
              />
              <p class="text-[11px] text-muted-foreground">
                对应飞书群机器人「签名校验」安全配置，填入后将自动生成 HMAC-SHA256 签名校验。
              </p>
            </div>

            <!-- 自定义关键词 -->
            <div class="space-y-1">
              <label class="text-xs font-medium text-slate-300">
                安全校验：自定义关键词 (Keyword)
              </label>
              <Input
                v-model="feishuForm.keyword"
                class="text-xs"
                placeholder="例如：抖音监控（未开启请留空）"
              />
              <p class="text-[11px] text-muted-foreground">
                对应飞书群机器人「自定义关键词」安全配置，发送时自动带有此前缀以通过安全过滤。
              </p>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- 运行与自启动 (shadcn Card) -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader class="pb-3">
        <div class="flex items-center gap-2">
          <Sliders class="w-4 h-4 text-sky-400" />
          <CardTitle>运行与自启设置</CardTitle>
        </div>
      </CardHeader>
      <CardContent class="space-y-3.5 pt-0">
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <div class="space-y-1">
            <label class="text-xs font-medium text-slate-300">页面加载等待时间（秒）</label>
            <Input v-model.number="form.page_wait_secs" type="number" min="2" max="60" />
            <p class="text-[11px] text-muted-foreground">页面打开后的缓冲等待，给数据提取预留渲染时间。</p>
          </div>

          <div class="space-y-1">
            <label class="text-xs font-medium text-slate-300">定时采集调度周期</label>
            <Input value="固定每 10 分钟自动调度" disabled class="opacity-60" />
            <p class="text-[11px] text-muted-foreground">内置最佳调度策略，每 10 分钟对监控作品轮巡采集。</p>
          </div>
        </div>

        <div class="pt-3 border-t border-border/60">
          <label class="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              :checked="form.autostart === true"
              :disabled="autostartBusy"
              class="rounded border-border"
              @change="toggleAutostart"
            />
            <span class="text-xs text-slate-200">
              开机自启动（登录 Windows 后在托盘后台静默就绪，不干扰日常桌面）
            </span>
          </label>
        </div>

        <div class="pt-2 flex items-center justify-between">
          <Button
            size="sm"
            variant="ghost"
            class="text-xs text-muted-foreground hover:text-white"
            @click="api.openDataDir().catch((err) => notify(String(err), 'error'))"
          >
            <FolderOpen class="w-3.5 h-3.5" />
            <span>打开本地数据目录</span>
          </Button>
          <Button
            size="sm"
            variant="ghost"
            class="text-xs text-muted-foreground hover:text-white"
            @click="api.openConfigFile().catch((err) => notify(String(err), 'error'))"
          >
            <FileCode class="w-3.5 h-3.5" />
            <span>打开配置文件 (config.toml)</span>
          </Button>
        </div>
      </CardContent>
    </Card>

    <!-- 数据存储与覆盖清理 (shadcn Card) -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader class="flex flex-row items-center justify-between pb-3">
        <div class="flex items-center gap-2">
          <Database class="w-4 h-4 text-emerald-400" />
          <CardTitle>数据存储与覆盖清理</CardTitle>
        </div>
        <Badge variant="outline" class="font-mono text-xs text-emerald-400 bg-emerald-500/10 border-emerald-500/20">
          存储占用：{{ fmtBytes(storageStats?.db_size_bytes) }}
        </Badge>
      </CardHeader>
      <CardContent class="space-y-4 pt-0">
        <!-- 存储状态概要统计 -->
        <div class="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
          <div class="p-2.5 rounded-lg bg-muted/20 border border-border/50">
            <div class="text-[11px] text-muted-foreground">数据库大小</div>
            <div class="text-base font-bold font-mono text-white mt-0.5">
              {{ fmtBytes(storageStats?.db_size_bytes) }}
            </div>
          </div>
          <div class="p-2.5 rounded-lg bg-muted/20 border border-border/50">
            <div class="text-[11px] text-muted-foreground">快照数据总量</div>
            <div class="text-base font-bold font-mono text-white mt-0.5">
              {{ fmtNum(storageStats?.snapshots_count) }} 条
            </div>
          </div>
          <div class="p-2.5 rounded-lg bg-muted/20 border border-border/50">
            <div class="text-[11px] text-muted-foreground">作品/监控总数</div>
            <div class="text-base font-bold font-mono text-white mt-0.5">
              {{ storageStats?.works_count ?? 0 }} / {{ storageStats?.monitored_count ?? 0 }}
            </div>
          </div>
          <div class="p-2.5 rounded-lg bg-muted/20 border border-border/50">
            <div class="text-[11px] text-muted-foreground">最早历史快照</div>
            <div class="text-base font-bold font-mono text-white mt-0.5">
              {{ fmtAgo(storageStats?.oldest_snapshot_ts) }}
            </div>
          </div>
        </div>

        <!-- 保留周期与自动滚动清理设置 -->
        <div class="space-y-3 pt-1">
          <div class="space-y-1.5">
            <div class="flex items-center justify-between">
              <label class="text-xs font-medium text-slate-300">
                快照数据保留周期（天）
              </label>
              <!-- 快捷预设按钮 -->
              <div class="flex items-center gap-1.5 text-xs">
                <button
                  type="button"
                  class="px-2 py-0.5 rounded border transition-all text-[11px]"
                  :class="form.retention_days === 7 ? 'bg-primary text-primary-foreground border-primary' : 'bg-muted/40 text-muted-foreground hover:text-white border-border/50'"
                  @click="form.retention_days = 7"
                >
                  7 天 (推荐)
                </button>
                <button
                  type="button"
                  class="px-2 py-0.5 rounded border transition-all text-[11px]"
                  :class="form.retention_days === 14 ? 'bg-primary text-primary-foreground border-primary' : 'bg-muted/40 text-muted-foreground hover:text-white border-border/50'"
                  @click="form.retention_days = 14"
                >
                  14 天
                </button>
                <button
                  type="button"
                  class="px-2 py-0.5 rounded border transition-all text-[11px]"
                  :class="form.retention_days === 30 ? 'bg-primary text-primary-foreground border-primary' : 'bg-muted/40 text-muted-foreground hover:text-white border-border/50'"
                  @click="form.retention_days = 30"
                >
                  30 天
                </button>
              </div>
            </div>
            <Input v-model.number="form.retention_days" type="number" min="1" max="365" class="h-8 max-w-xs font-mono" />
            <p class="text-[11px] text-muted-foreground">
              与作品 7 天趋势图协同运作，仅保留设定周期内的快照点，超出部分自动淘汰，防止长期运行占用大量磁盘。
            </p>
          </div>

          <label class="flex items-center gap-2 cursor-pointer pt-1">
            <input v-model="form.auto_clean" type="checkbox" class="rounded border-border" />
            <span class="text-xs text-slate-200">
              自动滚动覆盖清理（开启后每次定时采集完成自动淘汰超期快照，保持数据库常态轻量）
            </span>
          </label>
        </div>

        <!-- 立即执行覆盖清理操作栏 -->
        <div class="pt-3 border-t border-border/60 flex items-center justify-between">
          <div class="text-[11px] text-muted-foreground">
            手动清理将立即删除超期快照、未监控作品并执行 SQLite 磁盘空间整理 (VACUUM)。
          </div>
          <Button
            size="sm"
            variant="outline"
            class="text-xs text-amber-400 border-amber-500/30 hover:bg-amber-500/10 hover:text-amber-300"
            :disabled="cleaningStorage"
            @click="handleCleanStorage"
          >
            <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': cleaningStorage }" />
            <span>{{ cleaningStorage ? "正在覆盖清理..." : "一键覆盖清理与整理磁盘" }}</span>
          </Button>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
