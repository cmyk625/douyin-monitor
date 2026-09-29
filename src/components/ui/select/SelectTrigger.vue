<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { SelectIcon, SelectTrigger, type SelectTriggerProps, useForwardProps } from "radix-vue";
import { ChevronDown } from "lucide-vue-next";
import { cn } from "@/lib/utils";

const props = defineProps<SelectTriggerProps & { class?: HTMLAttributes["class"] }>();

const forwardedProps = useForwardProps(props);
</script>

<template>
  <SelectTrigger
    v-bind="forwardedProps"
    :class="
      cn(
        'flex h-8 w-full items-center justify-between gap-2 rounded-lg border border-slate-700 bg-slate-900/90 px-3 py-1 text-xs text-slate-100 shadow-sm transition-colors hover:border-slate-500 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/40 disabled:cursor-not-allowed disabled:opacity-50 [&>span]:line-clamp-1 cursor-pointer select-none',
        props.class,
      )
    "
  >
    <slot />
    <SelectIcon as-child>
      <ChevronDown class="h-3.5 w-3.5 opacity-60 shrink-0 transition-transform duration-200" />
    </SelectIcon>
  </SelectTrigger>
</template>
