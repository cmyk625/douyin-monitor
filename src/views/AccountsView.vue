<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/api";
import { notify, refreshAccounts, refreshStatus, setLoading, store } from "@/store";
import { fmtAgo, loginStateText } from "@/utils";
import type { Account, AccountInput } from "@/types";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
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
import { Chrome, Edit3, Plus, RefreshCw, Trash2 } from "lucide-vue-next";

const emit = defineEmits<{
  (e: "goto-works"): void;
}>();

const editing = ref<AccountInput | null>(null);

const hasChrome = computed(() => !!store.status?.chrome_path);

const CHROME_DOWNLOAD_URL = "https://www.google.cn/chrome/";

async function openChromeDownload(): Promise<void> {
  try {
    await api.openUrl(CHROME_DOWNLOAD_URL);
  } catch (err) {
    notify(`打开下载链接失败：${String(err)}`, "error");
  }
}

const blank = (): AccountInput => ({
  id: null,
  name: "",
  sec_uid: "",
  target_url: "https://www.douyin.com/",
  parse_script: "",
  enabled: true,
  note: "",
});

function openCreate(): void {
  editing.value = blank();
}

function openEdit(account: Account): void {
  editing.value = {
    id: account.id,
    name: account.name,
    sec_uid: account.sec_uid,
    target_url: account.target_url,
    parse_script: account.parse_script,
    enabled: account.enabled,
    note: account.note,
  };
}

function monitoredCount(accountId: number): number {
  return store.monitored.filter((item) => item.account_id === accountId).length;
}

async function save(): Promise<void> {
  if (!editing.value) return;
  if (!editing.value.name.trim()) {
    notify("请填写账号名称", "error");
    return;
  }
  setLoading("account-save", true);
  try {
    if (editing.value.id === null) {
      const account = await api.createAccount(editing.value);
      editing.value = null;
      await refreshAccounts();
      notify(`账号「${account.name}」已创建`, "ok");
      if (hasChrome.value) {
        await openLogin(account);
      } else {
        notify(`未检测到 Google Chrome 浏览器，无法自动拉起登录窗口。请前往 ${CHROME_DOWNLOAD_URL} 下载安装`, "warn");
      }
    } else {
      await api.updateAccount({ id: editing.value.id, name: editing.value.name.trim() });
      editing.value = null;
      notify("账号名称已更新", "ok");
      await refreshAccounts();
    }
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading("account-save", false);
  }
}

async function remove(account: Account): Promise<void> {
  if (!window.confirm(`确定删除账号「${account.name}」？关联的监控名单与快照数据会一并移除。`)) return;
  try {
    await api.deleteAccount(account.id);
    notify("账号已删除", "ok");
    await Promise.all([refreshAccounts(), refreshStatus()]);
  } catch (err) {
    notify(String(err), "error");
  }
}

async function openLogin(account: Account): Promise<void> {
  if (!hasChrome.value) {
    notify(`未检测到 Google Chrome 浏览器，正在打开官网下载地址：${CHROME_DOWNLOAD_URL}`, "warn");
    await openChromeDownload();
    return;
  }
  try {
    await api.openLoginWindow(account.id);
    notify(`已打开「${account.name}」的 Chrome 登录窗口，扫码完成后将自动同步状态`, "info");
  } catch (err) {
    notify(String(err), "error");
  }
}

async function runCheckLogin(account: Account): Promise<void> {
  if (!hasChrome.value) {
    notify(`未检测到 Google Chrome 浏览器，请前往 ${CHROME_DOWNLOAD_URL} 下载安装`, "error");
    return;
  }
  setLoading(`login-${account.id}`, true);
  try {
    const result = await api.checkLogin(account.id);
    notify(`${account.name}：${loginStateText(result.login_state)} · ${result.message}`, result.login_state === "ok" ? "ok" : "warn");
    await refreshAccounts();
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading(`login-${account.id}`, false);
  }
}

function badgeVariantForLogin(state: string) {
  if (state === "ok") return "success";
  if (state === "challenge") return "destructive";
  return "warning";
}

onMounted(() => {
  void refreshAccounts();
});
</script>

<template>
  <div class="h-full flex flex-col min-h-0 space-y-4">
    <div class="flex items-end justify-between gap-4 pb-1 shrink-0">
      <div>
        <h1 class="text-xl font-bold tracking-tight text-white">采集账号</h1>
        <p class="text-xs text-muted-foreground mt-1">
          管理用于访问抖音网页的 Chrome 会话环境。每个账号拥有独立的浏览器数据目录，扫码后持久保持登录。
        </p>
      </div>
      <div>
        <Button size="sm" @click="openCreate">
          <Plus class="w-3.5 h-3.5" />
          <span>新增采集账号</span>
        </Button>
      </div>
    </div>

    <!-- 账号列表数据表格 (shadcn Table) -->
    <Table containerClass="flex-1 min-h-0 overflow-auto">
      <TableHeader>
        <TableRow>
          <TableHead>账号名称</TableHead>
          <TableHead>登录状态</TableHead>
          <TableHead>监控作品数</TableHead>
          <TableHead>最近采集</TableHead>
          <TableHead>状态信息</TableHead>
          <TableHead class="w-1 text-center">操作</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="account in store.accounts" :key="account.id">
          <TableCell>
            <div class="font-medium text-white">{{ account.name }}</div>
            <div class="text-[11px] text-muted-foreground font-mono">ID #{{ account.id }}</div>
          </TableCell>

          <TableCell>
            <Badge :variant="badgeVariantForLogin(account.login_state)">
              {{ loginStateText(account.login_state) }}
            </Badge>
          </TableCell>

          <TableCell>
            <button
              v-if="monitoredCount(account.id) > 0"
              class="text-blue-400 hover:underline font-mono text-xs cursor-pointer"
              @click="emit('goto-works')"
            >
              {{ monitoredCount(account.id) }} 个视频
            </button>
            <Badge v-else variant="warning">未配置视频</Badge>
          </TableCell>

          <TableCell class="whitespace-nowrap text-muted-foreground font-mono text-xs">
            {{ fmtAgo(account.last_collect_at) }}
          </TableCell>

          <TableCell class="text-xs text-muted-foreground truncate max-w-[240px]" :title="account.last_error">
            {{ account.last_error || "会话正常" }}
          </TableCell>

          <TableCell>
            <div class="flex items-center justify-end gap-1.5">
              <Button
                size="sm"
                variant="outline"
                class="h-7 text-xs px-2"
                :title="!hasChrome ? `缺少 Chrome 浏览器，点击将前往下载：${CHROME_DOWNLOAD_URL}` : '打开独立 Chrome 窗口进行扫码登录'"
                @click="openLogin(account)"
              >
                <Chrome class="w-3 h-3 text-red-400" />
                <span>登录窗口</span>
              </Button>

              <Button
                size="sm"
                variant="ghost"
                class="h-7 text-xs px-2"
                :disabled="store.loading[`login-${account.id}`] || !hasChrome"
                title="检测当前账号登录凭证有效性"
                @click="runCheckLogin(account)"
              >
                <RefreshCw class="w-3 h-3" />
                <span>检测</span>
              </Button>

              <Button
                size="icon"
                variant="ghost"
                class="h-7 w-7"
                title="修改账号名称"
                @click="openEdit(account)"
              >
                <Edit3 class="w-3 h-3" />
              </Button>

              <Button
                size="icon"
                variant="destructive"
                class="h-7 w-7"
                title="删除账号"
                @click="remove(account)"
              >
                <Trash2 class="w-3 h-3" />
              </Button>
            </div>
          </TableCell>
        </TableRow>

        <TableEmpty v-if="store.accounts.length === 0" :colspan="6">
          还没有采集账号。点击右上角「新增采集账号」开始配置，创建后将自动打开 Chrome 扫码窗口。
        </TableEmpty>
      </TableBody>
    </Table>

    <!-- 创建 / 编辑账号弹窗 (shadcn Dialog, 右上角固定关闭按钮) -->
    <Dialog :open="!!editing" @update:open="(val) => { if (!val) editing = null; }">
      <DialogContent class="sm:max-w-md max-h-[90vh] flex flex-col p-0 overflow-hidden">
        <DialogHeader class="p-6 pb-3 pr-14 border-b border-border/40 shrink-0">
          <DialogTitle>{{ editing?.id === null ? "新增采集账号" : `编辑账号 #${editing?.id}` }}</DialogTitle>
          <DialogDescription>
            <template v-if="editing?.id === null">
              只需输入账号名称，保存后会自动拉起普通抖音网页版 Chrome 窗口扫码登录。
            </template>
            <template v-else>
              修改账号显示名称，原浏览器登录缓存依然保持有效。
            </template>
          </DialogDescription>
        </DialogHeader>

        <div v-if="editing" class="flex-1 overflow-y-auto p-6 space-y-4">
          <div class="space-y-1">
            <label class="text-xs font-medium text-slate-300">账号名称</label>
            <Input
              v-model="editing.name"
              placeholder="例如：主号 01 或 监控专用号"
              autofocus
              @keyup.enter="save"
            />
          </div>
        </div>

        <DialogFooter class="p-4 px-6 border-t border-border/40 shrink-0">
          <Button variant="ghost" size="sm" @click="editing = null">取消</Button>
          <Button
            size="sm"
            :disabled="store.loading['account-save']"
            @click="save"
          >
            {{ editing?.id === null ? "创建并打开登录" : "保存" }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
