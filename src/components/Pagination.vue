<script setup lang="ts">
import { computed, ref } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from "lucide-vue-next";

const props = withDefaults(
  defineProps<{
    currentPage: number;
    pageSize: number;
    totalItems: number;
    pageSizeOptions?: number[];
  }>(),
  {
    pageSizeOptions: () => [10, 15, 30, 50, 100],
  },
);

const emit = defineEmits<{
  (e: "update:currentPage", page: number): void;
  (e: "update:pageSize", size: number): void;
}>();

const totalPages = computed(() => Math.max(1, Math.ceil(props.totalItems / props.pageSize)));

const startItem = computed(() => {
  if (props.totalItems === 0) return 0;
  return (props.currentPage - 1) * props.pageSize + 1;
});

const endItem = computed(() => {
  return Math.min(props.totalItems, props.currentPage * props.pageSize);
});

const jumpInput = ref<string>("");

const selectedSizeStr = computed({
  get: () => String(props.pageSize),
  set: (val: string) => {
    const size = parseInt(val, 10);
    if (!isNaN(size) && size !== props.pageSize) {
      emit("update:pageSize", size);
      emit("update:currentPage", 1);
    }
  },
});

function setPage(p: number): void {
  const target = Math.max(1, Math.min(totalPages.value, p));
  if (target !== props.currentPage) {
    emit("update:currentPage", target);
  }
}

function handleJump(): void {
  const p = parseInt(jumpInput.value.trim(), 10);
  if (!isNaN(p)) {
    setPage(p);
  }
  jumpInput.value = "";
}
</script>

<template>
  <div class="flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground select-none py-1">
    <!-- 左侧：统计与美化后的每页条数选择 -->
    <div class="flex items-center gap-3">
      <span>
        显示 <span class="font-mono text-slate-200">{{ startItem }}-{{ endItem }}</span> 条，共 <span class="font-mono text-slate-200">{{ totalItems }}</span> 条
      </span>
      <div class="w-[110px]">
        <Select v-model="selectedSizeStr">
          <SelectTrigger class="h-7 text-xs px-2.5 bg-slate-900 border-slate-700/80">
            <SelectValue>
              {{ pageSize }} 条/页
            </SelectValue>
          </SelectTrigger>
          <SelectContent class="min-w-[110px]">
            <SelectItem v-for="opt in pageSizeOptions" :key="opt" :value="String(opt)">
              {{ opt }} 条/页
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <!-- 右侧：翻页控制与跳转 -->
    <div class="flex items-center gap-1.5">
      <Button
        variant="outline"
        size="icon"
        class="h-7 w-7 text-slate-300 disabled:opacity-40"
        :disabled="currentPage <= 1"
        title="首页"
        @click="setPage(1)"
      >
        <ChevronsLeft class="w-3.5 h-3.5" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        class="h-7 w-7 text-slate-300 disabled:opacity-40"
        :disabled="currentPage <= 1"
        title="上一页"
        @click="setPage(currentPage - 1)"
      >
        <ChevronLeft class="w-3.5 h-3.5" />
      </Button>

      <span class="px-2 font-mono text-slate-300 text-xs">
        第 <span class="text-white font-medium">{{ currentPage }}</span> / {{ totalPages }} 页
      </span>

      <Button
        variant="outline"
        size="icon"
        class="h-7 w-7 text-slate-300 disabled:opacity-40"
        :disabled="currentPage >= totalPages"
        title="下一页"
        @click="setPage(currentPage + 1)"
      >
        <ChevronRight class="w-3.5 h-3.5" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        class="h-7 w-7 text-slate-300 disabled:opacity-40"
        :disabled="currentPage >= totalPages"
        title="末页"
        @click="setPage(totalPages)"
      >
        <ChevronsRight class="w-3.5 h-3.5" />
      </Button>

      <!-- 快速跳页 -->
      <div v-if="totalPages > 1" class="flex items-center gap-1.5 ml-2">
        <span>跳至</span>
        <Input
          v-model="jumpInput"
          type="text"
          class="w-12 h-7 text-center text-xs bg-slate-900 border-slate-700/80 rounded px-1 text-slate-200 font-mono"
          :placeholder="String(currentPage)"
          @keydown.enter="handleJump"
        />
        <span>页</span>
      </div>
    </div>
  </div>
</template>
