<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api } from "@/api";
import { notify, refreshAlerts, refreshRules, setLoading, store } from "@/store";
import { METRIC_LABELS, fmtThreshold, fmtTime, parseThreshold } from "@/utils";
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
import { Edit3, Plus, RefreshCw, Trash2 } from "lucide-vue-next";
import type { Rule, RuleInput } from "@/types";

const editing = ref<RuleInput | null>(null);
const thresholdInput = ref("1000");
const ruleKeys = Object.entries(METRIC_LABELS);

const parsedThreshold = computed(() => parseThreshold(thresholdInput.value));

const selectedMetricLabel = computed(() => {
  if (!editing.value) return "选择指标";
  return METRIC_LABELS[editing.value.metric] || editing.value.metric;
});

const selectedAccountLabel = computed(() => {
  if (!editing.value || editing.value.account_id === null) return "全部账号 (全局)";
  const acc = store.accounts.find((a) => a.id === editing.value?.account_id);
  return acc ? acc.name : "全部账号 (全局)";
});

const blank = (): RuleInput => ({
  id: null,
  name: "点赞 30 分钟增长 100",
  account_id: null,
  metric: "like",
  window_minutes: 30,
  threshold: 100,
  enabled: true,
});

function openCreate(): void {
  editing.value = blank();
  thresholdInput.value = "100";
}

function openEdit(rule: Rule): void {
  editing.value = {
    id: rule.id,
    name: rule.name,
    account_id: rule.account_id,
    metric: rule.metric,
    window_minutes: rule.window_minutes,
    threshold: rule.threshold,
    enabled: rule.enabled,
  };
  if (rule.threshold >= 10_000 && rule.threshold % 1000 === 0) {
    const wan = (rule.threshold / 10_000).toFixed(1).replace(/\.0$/, "");
    thresholdInput.value = `${wan}万`;
  } else {
    thresholdInput.value = String(rule.threshold);
  }
}

async function save(): Promise<void> {
  if (!editing.value) return;
  const input = editing.value;
  if (!input.name.trim()) {
    notify("请填写规则名称", "error");
    return;
  }
  const parsed = parseThreshold(thresholdInput.value);
  if (!parsed || parsed < 1) {
    notify("请输入有效的增量阈值（如 1000、1万、0.5万）", "error");
    return;
  }
  input.threshold = parsed;

  if (input.window_minutes < 1 || input.threshold < 1) {
    notify("窗口与阈值必须为正整数", "error");
    return;
  }
  setLoading("rule-save", true);
  try {
    await api.saveRule(input);
    editing.value = null;
    await refreshRules();
    notify("规则已保存", "ok");
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading("rule-save", false);
  }
}

async function toggle(rule: Rule): Promise<void> {
  try {
    await api.saveRule({
      id: rule.id,
      name: rule.name,
      account_id: rule.account_id,
      metric: rule.metric,
      window_minutes: rule.window_minutes,
      threshold: rule.threshold,
      enabled: !rule.enabled,
    });
    await refreshRules();
  } catch (err) {
    notify(String(err), "error");
  }
}

async function remove(rule: Rule): Promise<void> {
  if (!window.confirm(`确定删除规则「${rule.name}」？`)) return;
  try {
    await api.deleteRule(rule.id);
    await refreshRules();
    notify("规则已删除", "ok");
  } catch (err) {
    notify(String(err), "error");
  }
}

async function run(): Promise<void> {
  setLoading("rule-run", true);
  try {
    const count = await api.runRules();
    await Promise.all([refreshAlerts(), refreshRules()]);
    notify(count > 0 ? `命中 ${count} 条报警，已推送飞书` : "本次未命中任何规则", count > 0 ? "ok" : "info");
  } catch (err) {
    notify(String(err), "error");
  } finally {
    setLoading("rule-run", false);
  }
}

onMounted(() => {
  void refreshRules();
});
</script>

<template>
  <div class="h-full flex flex-col min-h-0 space-y-4">
    <div class="flex items-end justify-between gap-4 pb-1 shrink-0">
      <div>
        <h1 class="text-xl font-bold tracking-tight text-white">报警规则</h1>
        <p class="text-xs text-muted-foreground mt-1">基于 [时间窗口增量] 自动判定：当前采集值 - 窗口起点快照 ≥ 阈值时触发报警并推送到飞书。</p>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" :disabled="store.loading['rule-run']" @click="run">
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': store.loading['rule-run'] }" />
          <span>立即评估规则</span>
        </Button>
        <Button size="sm" @click="openCreate">
          <Plus class="w-3.5 h-3.5" />
          <span>新建规则</span>
        </Button>
      </div>
    </div>

    <!-- 规则数据表格 (shadcn Table) -->
    <Table containerClass="flex-1 min-h-0 overflow-auto">
      <TableHeader>
        <TableRow>
          <TableHead>规则名称</TableHead>
          <TableHead>适用账号</TableHead>
          <TableHead>监控指标</TableHead>
          <TableHead>时间跨度</TableHead>
          <TableHead>增长阈值</TableHead>
          <TableHead>规则状态</TableHead>
          <TableHead>创建时间</TableHead>
          <TableHead class="w-1 text-center">操作</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="rule in store.rules" :key="rule.id">
          <TableCell class="font-medium text-white">{{ rule.name }}</TableCell>
          <TableCell class="text-muted-foreground text-xs whitespace-nowrap">
            {{ rule.account_name || "全部账号 (全局)" }}
          </TableCell>
          <TableCell>
            <Badge variant="secondary" class="font-mono text-[11px]">
              {{ METRIC_LABELS[rule.metric] || rule.metric }}
            </Badge>
          </TableCell>
          <TableCell class="text-muted-foreground font-mono text-xs whitespace-nowrap">
            {{ rule.window_minutes }} 分钟
          </TableCell>
          <TableCell class="font-mono font-semibold text-emerald-400 whitespace-nowrap">
            +{{ fmtThreshold(rule.threshold) }}
          </TableCell>
          <TableCell>
            <Badge :variant="rule.enabled ? 'success' : 'secondary'">
              {{ rule.enabled ? "生效中" : "已停用" }}
            </Badge>
          </TableCell>
          <TableCell class="whitespace-nowrap text-muted-foreground font-mono text-[11px]">
            {{ fmtTime(rule.created_at) }}
          </TableCell>
          <TableCell>
            <div class="flex items-center justify-end gap-1.5">
              <Button size="sm" variant="ghost" class="h-7 text-xs px-2" @click="toggle(rule)">
                {{ rule.enabled ? "停用" : "启用" }}
              </Button>
              <Button size="icon" variant="ghost" class="h-7 w-7" @click="openEdit(rule)">
                <Edit3 class="w-3 h-3" />
              </Button>
              <Button size="icon" variant="destructive" class="h-7 w-7" @click="remove(rule)">
                <Trash2 class="w-3 h-3" />
              </Button>
            </div>
          </TableCell>
        </TableRow>

        <TableEmpty v-if="store.rules.length === 0" :colspan="8">
          还没有规则。例如：点赞在 30 分钟内增长达到 100 时通知飞书。
        </TableEmpty>
      </TableBody>
    </Table>

    <!-- 新建 / 编辑规则弹窗 (shadcn Dialog, 右上角固定关闭按钮) -->
    <Dialog :open="!!editing" @update:open="(val) => { if (!val) editing = null; }">
      <DialogContent class="sm:max-w-lg max-h-[90vh] flex flex-col p-0 overflow-hidden">
        <DialogHeader class="p-6 pb-3 pr-14 border-b border-border/40 shrink-0">
          <DialogTitle>{{ editing?.id === null ? "新建规则" : `编辑规则 #${editing?.id}` }}</DialogTitle>
          <DialogDescription>
            只需设置监控时间与增长阈值。周期内增长达到阈值即刻触发通知；通知完毕后，下一个周期内再次增长达到阈值才会再次通知。
          </DialogDescription>
        </DialogHeader>

        <div v-if="editing" class="flex-1 overflow-y-auto p-6 space-y-3.5">
          <div class="space-y-1.5">
            <label class="text-xs font-medium text-slate-300">规则名称</label>
            <Input v-model="editing.name" />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-slate-300">监控指标</label>
              <Select v-model="editing.metric">
                <SelectTrigger>
                  <SelectValue placeholder="选择指标">
                    {{ selectedMetricLabel }}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="[key, label] in ruleKeys" :key="key" :value="key">
                    {{ label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-slate-300">适用账号范围</label>
              <Select
                :model-value="editing.account_id === null ? 'all' : String(editing.account_id)"
                @update:model-value="editing.account_id = $event === 'all' ? null : Number($event)"
              >
                <SelectTrigger>
                  <SelectValue placeholder="全部账号">
                    {{ selectedAccountLabel }}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">全部账号 (全局)</SelectItem>
                  <SelectItem
                    v-for="account in store.accounts"
                    :key="account.id"
                    :value="String(account.id)"
                  >
                    {{ account.name }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-slate-300">时间跨度（分钟）</label>
              <Input v-model.number="editing.window_minutes" type="number" min="1" placeholder="例如：30" />
              <p class="text-[11px] text-muted-foreground">例如：30 分钟</p>
            </div>
            <div class="space-y-1.5">
              <label class="text-xs font-medium text-slate-300">增长阈值</label>
              <Input
                v-model="thresholdInput"
                placeholder="例如：100 或 1000、1万"
              />
              <p v-if="parsedThreshold !== null && (thresholdInput.includes('万') || thresholdInput.includes('w') || thresholdInput.includes('W') || parsedThreshold >= 10000)" class="text-[10px] text-emerald-400 font-mono">
                换算为：{{ parsedThreshold.toLocaleString() }}
              </p>
              <p v-else-if="thresholdInput.trim() && parsedThreshold === null" class="text-[10px] text-rose-400">
                请输入有效阈值（如 100、1000）
              </p>
              <p v-else class="text-[11px] text-muted-foreground">例如：增长 100</p>
            </div>
          </div>

          <label class="flex items-center gap-2 cursor-pointer pt-1">
            <input v-model="editing.enabled" type="checkbox" class="rounded border-border" />
            <span class="text-xs text-slate-200">启用此规则</span>
          </label>
          <div class="p-2.5 rounded-lg bg-sky-950/20 border border-sky-500/20 text-sky-300 text-[11px] leading-relaxed">
            💡 <strong>周期自动调度逻辑</strong>：例如设定 30 分钟阈值 100，即在 30 分钟内增长 100 个即刻向飞书推送告警；通知完毕后，自动进入下一个 30 分钟周期，下个周期内再次增长 100 个才会再次通知，彻底杜绝重复骚扰。
          </div>
        </div>

        <DialogFooter class="p-4 px-6 border-t border-border/40 shrink-0">
          <Button variant="ghost" size="sm" @click="editing = null">取消</Button>
          <Button size="sm" :disabled="store.loading['rule-save']" @click="save">保存规则</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
