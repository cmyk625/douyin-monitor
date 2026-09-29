<script setup lang="ts">
import { ref } from "vue";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Bell,
  BookOpen,
  ChevronDown,
  ChevronUp,
  Chrome,
  Clock,
  ExternalLink,
  HelpCircle,
  Key,
  Layers,
  Lightbulb,
  Send,
  Settings,
  ShieldAlert,
  ShieldCheck,
  Users,
  Video,
  Zap,
} from "lucide-vue-next";

const emit = defineEmits<{
  (e: "goto", tab: string): void;
}>();

const activeFaq = ref<number | null>(null);

function toggleFaq(index: number): void {
  activeFaq.value = activeFaq.value === index ? null : index;
}

const faqs = [
  {
    q: "提示“未检测到 Chrome 浏览器”或采集失败怎么处理？",
    a: "本应用依赖正版 Google Chrome 进行数据提取。请先前往 Chrome 官网 (https://www.google.cn/chrome/) 下载并安装。若已安装在非默认盘符（如 D 盘），可在「系统设置」->「Google Chrome 运行环境」输入 chrome.exe 的完整绝对路径，然后点击“重新检测”即可立即生效。",
  },
  {
    q: "飞书测试连接失败，提示“code: 19001 / sign match fail”或“keyword not found”？",
    a: "这是因为飞书机器人的群安全设置不匹配导致：1) 若机器人在飞书开启了「签名校验」，请在软件「系统设置」->「飞书高级设置」中填入对应的 Secret 密钥；2) 若开启了「自定义关键词」，请在「自定义关键词」输入框填入匹配的词汇；3) 若测试无需安全校验，可临时在飞书群机器人设置中取消勾选这两项。",
  },
  {
    q: "为什么建议一定要在「采集账号」中扫码登录？",
    a: "抖音 Web 页面若处于未登录的访客模式下，高频请求很容易触发反爬机制或弹出滑块验证码；扫码登录后携带合法 Cookie 凭证，提取成功率达 99% 以上。并且每个账号独享独立的 profile 目录，互不串号、互不干扰。",
  },
  {
    q: "采集时若偶发滑块验证码，如何手动通过？",
    a: "在「采集账号」列表点击对应账号右侧的「打开登录窗口」，系统会弹出该账号专用的真实浏览器窗口。在窗口中手动完成滑块拼图验证后关闭窗口即可，Cookie 会自动持久化保存在本地。",
  },
  {
    q: "软件关闭后还会继续采集和报警吗？",
    a: "会！点击右上角关闭按钮时，程序会自动最小化到 Windows 右下角托盘区，并在后台按 10 分钟定时调度采集。此外建议在「系统设置」中开启「开机自启动」，电脑每次开机后将在后台静默就绪，无需人工干预。",
  },
  {
    q: "增量报警规则是如何计算的？为什么移除了播放量？",
    a: "系统采用历史快照比对算法：例如设定“点赞量 10 分钟增量 >= 500”，系统会查找 10 分钟前采集到的快照点作为基准 (baseline)，当前值减去基准值如果达到 500 则立即触发告警。根据抖音运营实际情况，视频在网页端难以直接获取精准实时逐秒播放量，点赞/评论/分享/收藏等核心互动数据更真实反映爆发趋势，故报警聚焦于关键指标。",
  },
];
</script>

<template>
  <div class="h-full overflow-y-auto space-y-5 pr-1">
    <!-- 头部说明横幅 -->
    <div class="relative overflow-hidden rounded-xl border border-sky-500/20 bg-gradient-to-r from-sky-950/40 via-slate-900/60 to-slate-900/90 p-5 shadow-lg">
      <div class="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div class="flex items-center gap-2 mb-1.5">
            <Badge variant="outline" class="border-sky-500/30 text-sky-400 bg-sky-500/10 text-xs">
              零基础快速上手
            </Badge>
            <span class="text-xs text-slate-400">持续更新 · 抖音爆款监控系统</span>
          </div>
          <h1 class="text-xl font-bold tracking-tight text-white flex items-center gap-2">
            <BookOpen class="w-5 h-5 text-sky-400" />
            <span>抖音作品监控使用教程与配置指南</span>
          </h1>
          <p class="text-xs text-slate-300 mt-1 max-w-2xl leading-relaxed">
            从本地运行环境检查、抖音账号安全授权、作品名单录入，到智能增量报警与飞书群卡片推送的全流程图文使用说明。
          </p>
        </div>
        <div class="flex items-center gap-2 shrink-0 flex-wrap">
          <Button size="sm" variant="outline" class="text-xs" @click="emit('goto', 'settings')">
            <Settings class="w-3.5 h-3.5" />
            <span>系统设置</span>
          </Button>
          <Button size="sm" class="text-xs bg-sky-600 hover:bg-sky-500 text-white" @click="emit('goto', 'works')">
            <Video class="w-3.5 h-3.5" />
            <span>录入监控作品</span>
          </Button>
        </div>
      </div>
    </div>

    <!-- 核心全流程极简步骤卡片导航 -->
    <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-2.5">
      <div
        class="p-3 rounded-lg border border-border/60 bg-card/60 hover:border-sky-500/40 hover:bg-sky-500/5 transition-all cursor-pointer group"
        @click="emit('goto', 'settings')"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-[10px] font-mono font-bold text-sky-400 bg-sky-500/10 px-1.5 py-0.5 rounded">01</span>
          <ShieldCheck class="w-4 h-4 text-sky-400 group-hover:scale-110 transition-transform" />
        </div>
        <div class="text-xs font-semibold text-slate-200">环境准备</div>
        <p class="text-[10px] text-muted-foreground mt-0.5">WebView2 与 Chrome 就绪</p>
      </div>

      <div
        class="p-3 rounded-lg border border-border/60 bg-card/60 hover:border-emerald-500/40 hover:bg-emerald-500/5 transition-all cursor-pointer group"
        @click="emit('goto', 'accounts')"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-[10px] font-mono font-bold text-emerald-400 bg-emerald-500/10 px-1.5 py-0.5 rounded">02</span>
          <Users class="w-4 h-4 text-emerald-400 group-hover:scale-110 transition-transform" />
        </div>
        <div class="text-xs font-semibold text-slate-200">账号扫码</div>
        <p class="text-[10px] text-muted-foreground mt-0.5">扫码登录与独立沙箱</p>
      </div>

      <div
        class="p-3 rounded-lg border border-border/60 bg-card/60 hover:border-blue-500/40 hover:bg-blue-500/5 transition-all cursor-pointer group"
        @click="emit('goto', 'works')"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-[10px] font-mono font-bold text-blue-400 bg-blue-500/10 px-1.5 py-0.5 rounded">03</span>
          <Video class="w-4 h-4 text-blue-400 group-hover:scale-110 transition-transform" />
        </div>
        <div class="text-xs font-semibold text-slate-200">作品录入</div>
        <p class="text-[10px] text-muted-foreground mt-0.5">链接自动清洗与负责人</p>
      </div>

      <div
        class="p-3 rounded-lg border border-border/60 bg-card/60 hover:border-purple-500/40 hover:bg-purple-500/5 transition-all cursor-pointer group"
        @click="emit('goto', 'rules')"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-[10px] font-mono font-bold text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded">04</span>
          <ShieldAlert class="w-4 h-4 text-purple-400 group-hover:scale-110 transition-transform" />
        </div>
        <div class="text-xs font-semibold text-slate-200">报警规则</div>
        <p class="text-[10px] text-muted-foreground mt-0.5">点赞/评论等增量阈值</p>
      </div>

      <div
        class="p-3 rounded-lg border border-border/60 bg-card/60 hover:border-amber-500/40 hover:bg-amber-500/5 transition-all cursor-pointer group"
        @click="emit('goto', 'settings')"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-[10px] font-mono font-bold text-amber-400 bg-amber-500/10 px-1.5 py-0.5 rounded">05</span>
          <Send class="w-4 h-4 text-amber-400 group-hover:scale-110 transition-transform" />
        </div>
        <div class="text-xs font-semibold text-slate-200">飞书推送</div>
        <p class="text-[10px] text-muted-foreground mt-0.5">Webhook 与安全校验</p>
      </div>

      <div
        class="p-3 rounded-lg border border-border/60 bg-card/60 hover:border-teal-500/40 hover:bg-teal-500/5 transition-all cursor-pointer group"
        @click="emit('goto', 'alerts')"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-[10px] font-mono font-bold text-teal-400 bg-teal-500/10 px-1.5 py-0.5 rounded">06</span>
          <Bell class="w-4 h-4 text-teal-400 group-hover:scale-110 transition-transform" />
        </div>
        <div class="text-xs font-semibold text-slate-200">通知记录</div>
        <p class="text-[10px] text-muted-foreground mt-0.5">报警回溯与补发</p>
      </div>
    </div>

    <!-- 详细分步教程 -->
    <div class="space-y-4">
      <!-- 步骤 1 -->
      <Card class="bg-card/70 border-border/70">
        <CardHeader class="pb-3 border-b border-border/30 bg-muted/10">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="w-6 h-6 rounded-full bg-sky-500/20 text-sky-400 text-xs font-bold flex items-center justify-center border border-sky-500/30">
                1
              </span>
              <div>
                <CardTitle class="text-sm font-semibold text-white">运行环境准备与体检</CardTitle>
                <p class="text-[11px] text-muted-foreground">保障软件底层组件正常运作，避免数据抓取中断</p>
              </div>
            </div>
            <Button size="sm" variant="outline" class="text-xs" @click="emit('goto', 'settings')">
              前往环境体检
            </Button>
          </div>
        </CardHeader>
        <CardContent class="p-4 space-y-3 text-xs leading-relaxed text-slate-300">
          <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div class="p-3 rounded-lg border border-border/40 bg-muted/10">
              <div class="font-medium text-white flex items-center gap-1.5 mb-1">
                <Chrome class="w-4 h-4 text-blue-400" />
                <span>Google Chrome 浏览器</span>
              </div>
              <p class="text-muted-foreground text-[11px]">
                数据采集依托官方 Chrome 的自动化引擎。若本机尚未安装 Chrome，可前往
                <a
                  href="https://www.google.cn/chrome/"
                  target="_blank"
                  class="text-sky-400 hover:underline inline-flex items-center gap-0.5"
                >
                  google.cn/chrome <ExternalLink class="w-2.5 h-2.5" />
                </a>
                下载。默认自动寻找标准路径；若装在特殊目录，可在设置中指定路径。
              </p>
            </div>
            <div class="p-3 rounded-lg border border-border/40 bg-muted/10">
              <div class="font-medium text-white flex items-center gap-1.5 mb-1">
                <ShieldCheck class="w-4 h-4 text-emerald-400" />
                <span>Microsoft Edge WebView2</span>
              </div>
              <p class="text-muted-foreground text-[11px]">
                软件客户端界面基于 WebView2 渲染。现代 Windows 10/11 系统均已内置。若系统精简版提示缺失，可在「系统设置」中点击一键下载官方引导安装程序进行修复。
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- 步骤 2 -->
      <Card class="bg-card/70 border-border/70">
        <CardHeader class="pb-3 border-b border-border/30 bg-muted/10">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="w-6 h-6 rounded-full bg-emerald-500/20 text-emerald-400 text-xs font-bold flex items-center justify-center border border-emerald-500/30">
                2
              </span>
              <div>
                <CardTitle class="text-sm font-semibold text-white">添加采集账号与扫码登录</CardTitle>
                <p class="text-[11px] text-muted-foreground">建立合规登录会话，有效降低滑块验证概率</p>
              </div>
            </div>
            <Button size="sm" variant="outline" class="text-xs" @click="emit('goto', 'accounts')">
              前往账号管理
            </Button>
          </div>
        </CardHeader>
        <CardContent class="p-4 space-y-3 text-xs leading-relaxed text-slate-300">
          <ol class="list-decimal list-inside space-y-1.5 text-slate-300">
            <li>
              点击导航栏的<strong>「采集账号」</strong>，点击<strong>「添加账号」</strong>，输入账号备注（如“监控小号 1”）。
            </li>
            <li>
              在列表中对应账号右侧点击<strong>「打开登录窗口」</strong>，系统将唤起专属独立的浏览器沙箱窗口。
            </li>
            <li>
              使用手机打开<strong>抖音 App -> 扫一扫</strong>，扫描窗口中的二维码并确认登录。
            </li>
            <li>
              登录成功后，无需任何额外保存操作，直接关闭或最小化该登录窗口，应用后台会自动识别并更新为<span class="text-emerald-400 font-medium">「登录正常」</span>。
            </li>
          </ol>
          <div class="p-2.5 rounded-lg bg-emerald-950/20 border border-emerald-500/20 text-emerald-300 text-[11px] flex items-center gap-2">
            <Lightbulb class="w-4 h-4 shrink-0 text-emerald-400" />
            <span>每个账号使用专属数据目录隔离存储 Cookie，支持多账号并存，会话过期后直接点击「打开登录窗口」重新扫码即可。</span>
          </div>
        </CardContent>
      </Card>

      <!-- 步骤 3 -->
      <Card class="bg-card/70 border-border/70">
        <CardHeader class="pb-3 border-b border-border/30 bg-muted/10">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="w-6 h-6 rounded-full bg-blue-500/20 text-blue-400 text-xs font-bold flex items-center justify-center border border-blue-500/30">
                3
              </span>
              <div>
                <CardTitle class="text-sm font-semibold text-white">录入作品监控名单与指定负责人</CardTitle>
                <p class="text-[11px] text-muted-foreground">精准锁定监控目标，支持长文案分享口令智能解析</p>
              </div>
            </div>
            <Button size="sm" variant="outline" class="text-xs" @click="emit('goto', 'works')">
              前往监控作品
            </Button>
          </div>
        </CardHeader>
        <CardContent class="p-4 space-y-3 text-xs leading-relaxed text-slate-300">
          <div class="space-y-2">
            <p>在<strong>「监控作品」</strong>页面点击<strong>「添加监控作品」</strong>，填入作品信息：</p>
            <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div class="p-3 rounded-lg border border-border/40 bg-muted/15 space-y-1.5">
                <div class="font-medium text-white flex items-center gap-1.5">
                  <Video class="w-3.5 h-3.5 text-blue-400" />
                  <span>支持多种链接格式（自动清洗）</span>
                </div>
                <p class="text-[11px] text-muted-foreground">
                  直接粘贴抖音 App 分享的整段文案（如：<code>7.28 复制打开抖音，看看【xxx的作品】... https://v.douyin.com/xxx/</code>）或电脑网页端链接（如 <code>https://www.douyin.com/video/7123456789...</code>），系统会自动提取纯净有效链接。
                </p>
              </div>

              <div class="p-3 rounded-lg border border-border/40 bg-muted/15 space-y-1.5">
                <div class="font-medium text-white flex items-center gap-1.5">
                  <Users class="w-3.5 h-3.5 text-emerald-400" />
                  <span>指定负责人（带快捷记忆与删除）</span>
                </div>
                <p class="text-[11px] text-muted-foreground">
                  输入该视频的责任人姓名（如“张三”）。下次录入时聚焦输入框将自动弹出历史保存的负责人，点击一键填入；若某负责人已不再需要，点击右侧的 <strong>×</strong> 按钮即可从快捷列表中移除。
                </p>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- 步骤 4 -->
      <Card class="bg-card/70 border-border/70">
        <CardHeader class="pb-3 border-b border-border/30 bg-muted/10">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="w-6 h-6 rounded-full bg-purple-500/20 text-purple-400 text-xs font-bold flex items-center justify-center border border-purple-500/30">
                4
              </span>
              <div>
                <CardTitle class="text-sm font-semibold text-white">配置智能增量报警规则</CardTitle>
                <p class="text-[11px] text-muted-foreground">设定时间窗口与指标增量阈值，捕捉爆款起飞信号</p>
              </div>
            </div>
            <Button size="sm" variant="outline" class="text-xs" @click="emit('goto', 'rules')">
              前往报警规则
            </Button>
          </div>
        </CardHeader>
        <CardContent class="p-4 space-y-3 text-xs leading-relaxed text-slate-300">
          <div class="grid grid-cols-1 sm:grid-cols-3 gap-2.5">
            <div class="p-2.5 rounded-lg border border-border/40 bg-muted/10">
              <div class="text-[11px] text-muted-foreground">1. 监控指标</div>
              <div class="text-sm font-semibold text-white mt-1">点赞 · 评论 · 分享 · 收藏</div>
              <p class="text-[10px] text-muted-foreground mt-1">剔除虚高无法逐秒精确提取的播放量，聚焦真实互动。</p>
            </div>
            <div class="p-2.5 rounded-lg border border-border/40 bg-muted/10">
              <div class="text-[11px] text-muted-foreground">2. 时间窗口</div>
              <div class="text-sm font-semibold text-white mt-1">10m / 30m / 60m / 120m</div>
              <p class="text-[10px] text-muted-foreground mt-1">自动查找窗口起始点附近的历史快照作为 baseline。</p>
            </div>
            <div class="p-2.5 rounded-lg border border-border/40 bg-muted/10">
              <div class="text-[11px] text-muted-foreground">3. 差量计算与通知</div>
              <div class="text-sm font-semibold text-white mt-1">增量 ≥ 阈值 即刻告警</div>
              <p class="text-[10px] text-muted-foreground mt-1">当前值 - 基准值超过阈值即生成报警卡片推送到飞书。</p>
            </div>
          </div>
          <div class="p-2.5 rounded-lg bg-purple-950/20 border border-purple-500/20 text-purple-300 text-[11px] flex items-center gap-2">
            <Lightbulb class="w-4 h-4 shrink-0 text-purple-400" />
            <span><strong>自动防重周期</strong>：无需单独配置繁琐的冷却时间。例如设定 30 分钟阈值 100：在 30 分钟内达到 100 增量即刻告警；告警完毕后，自动进入下一个 30 分钟周期，只有再次增长达到 100 才会再次通知。</span>
          </div>
        </CardContent>
      </Card>

      <!-- 步骤 5 -->
      <Card class="bg-card/70 border-border/70">
        <CardHeader class="pb-3 border-b border-border/30 bg-muted/10">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="w-6 h-6 rounded-full bg-amber-500/20 text-amber-400 text-xs font-bold flex items-center justify-center border border-amber-500/30">
                5
              </span>
              <div>
                <CardTitle class="text-sm font-semibold text-white">飞书群自定义机器人 Webhook 配置与测试</CardTitle>
                <p class="text-[11px] text-muted-foreground">全自动群聊卡片告警，团队实时同步</p>
              </div>
            </div>
            <Button size="sm" variant="outline" class="text-xs" @click="emit('goto', 'settings')">
              前往配置飞书
            </Button>
          </div>
        </CardHeader>
        <CardContent class="p-4 space-y-3.5 text-xs leading-relaxed text-slate-300">
          <div class="space-y-2">
            <div class="font-medium text-white">4 步极速接入飞书群机器人：</div>
            <ol class="list-decimal list-inside space-y-1 text-slate-300">
              <li>在飞书电脑端打开接收报警的飞书群，点击群右上角<strong>「设置 (齿轮)」</strong>。</li>
              <li>选择<strong>「群机器人」</strong>-> 点击<strong>「添加机器人」</strong>-> 选择<strong>「自定义机器人」</strong>。</li>
              <li>为机器人起名（如“抖音监控预警”），点击完成，复制生成的 <code>Webhook 地址</code>。</li>
              <li>回到本软件<strong>「系统设置」</strong>，将 Webhook 粘贴到「飞书 Webhook 地址 (URL)」输入框中。</li>
            </ol>
          </div>

          <div class="p-3 rounded-lg border border-border/50 bg-slate-900/60 space-y-2">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-1.5 font-medium text-amber-400">
                <Key class="w-3.5 h-3.5" />
                <span>安全设置说明（在「系统设置」->「飞书高级设置」中配置）</span>
              </div>
            </div>
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-2 text-[11px]">
              <div class="p-2 rounded bg-muted/20 border border-border/40">
                <span class="font-semibold text-slate-200">1. 签名校验 (Secret)：</span>
                <p class="text-muted-foreground mt-0.5">
                  若飞书机器人勾选了“签名校验”，将飞书生成的 Secret 粘贴到软件的签名密钥框中。软件自动执行 HMAC-SHA256 签名，未勾选请留空。
                </p>
              </div>
              <div class="p-2 rounded bg-muted/20 border border-border/40">
                <span class="font-semibold text-slate-200">2. 自定义关键词 (Keyword)：</span>
                <p class="text-muted-foreground mt-0.5">
                  若飞书机器人勾选了“自定义关键词”，在软件对应框中填入该词（如“抖音监控”）。推送消息会自动带上此前缀以通过飞书安全验证。
                </p>
              </div>
            </div>
            <div class="pt-1 flex items-center justify-between text-[11px]">
              <span class="text-muted-foreground">配置完成后，点击输入框旁的「测试连接」，飞书群能收到测试消息即大功告成！</span>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- 步骤 6 -->
      <Card class="bg-card/70 border-border/70">
        <CardHeader class="pb-3 border-b border-border/30 bg-muted/10">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="w-6 h-6 rounded-full bg-teal-500/20 text-teal-400 text-xs font-bold flex items-center justify-center border border-teal-500/30">
                6
              </span>
              <div>
                <CardTitle class="text-sm font-semibold text-white">开机自启、后台静默与数据自动维护</CardTitle>
                <p class="text-[11px] text-muted-foreground">无人值守常态化稳定运行</p>
              </div>
            </div>
            <Button size="sm" variant="outline" class="text-xs" @click="emit('goto', 'settings')">
              前往自启设置
            </Button>
          </div>
        </CardHeader>
        <CardContent class="p-4 space-y-3 text-xs leading-relaxed text-slate-300">
          <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
            <div class="p-3 rounded-lg border border-border/40 bg-muted/10 space-y-1">
              <div class="font-medium text-white flex items-center gap-1.5">
                <Zap class="w-3.5 h-3.5 text-amber-400" />
                <span>开机自启动</span>
              </div>
              <p class="text-[11px] text-muted-foreground">
                开启后，开机登录 Windows 自动在托盘后台拉起，主界面不弹出打扰工作，保持静默采集。
              </p>
            </div>
            <div class="p-3 rounded-lg border border-border/40 bg-muted/10 space-y-1">
              <div class="font-medium text-white flex items-center gap-1.5">
                <Clock class="w-3.5 h-3.5 text-sky-400" />
                <span>每 10 分钟自动调度</span>
              </div>
              <p class="text-[11px] text-muted-foreground">
                经过长期验证的最佳调度频率，既能敏锐捕捉作品起飞波峰，又极大避免因过于频繁被风控。
              </p>
            </div>
            <div class="p-3 rounded-lg border border-border/40 bg-muted/10 space-y-1">
              <div class="font-medium text-white flex items-center gap-1.5">
                <Layers class="w-3.5 h-3.5 text-emerald-400" />
                <span>7 天自动滚动覆盖清理</span>
              </div>
              <p class="text-[11px] text-muted-foreground">
                默认自动滚动清理 7 天前旧快照，执行 SQLite VACUUM 释放磁盘，保持数据库始终轻量小巧。
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>

    <!-- 常见问题排查 (FAQ) -->
    <Card class="bg-card/70 border-border/70">
      <CardHeader class="pb-3 border-b border-border/30 bg-muted/10">
        <div class="flex items-center gap-2">
          <HelpCircle class="w-4 h-4 text-sky-400" />
          <div>
            <CardTitle class="text-sm font-semibold text-white">常见问题与排查指南 (FAQ)</CardTitle>
            <p class="text-[11px] text-muted-foreground">快速解答日常使用中的疑问与异常处置</p>
          </div>
        </div>
      </CardHeader>
      <CardContent class="p-4 divide-y divide-border/40 pt-1">
        <div
          v-for="(faq, idx) in faqs"
          :key="idx"
          class="py-3 first:pt-2 last:pb-1"
        >
          <button
            type="button"
            class="w-full flex items-center justify-between text-left text-xs font-medium text-slate-200 hover:text-white transition-colors cursor-pointer"
            @click="toggleFaq(idx)"
          >
            <span class="flex items-center gap-2">
              <span class="text-sky-400 font-mono text-[11px]">Q{{ idx + 1 }}.</span>
              <span>{{ faq.q }}</span>
            </span>
            <component
              :is="activeFaq === idx ? ChevronUp : ChevronDown"
              class="w-4 h-4 text-muted-foreground shrink-0 ml-2"
            />
          </button>
          <div
            v-if="activeFaq === idx"
            class="mt-2 text-xs text-muted-foreground leading-relaxed pl-6 animate-in fade-in-0 duration-150"
          >
            {{ faq.a }}
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>
