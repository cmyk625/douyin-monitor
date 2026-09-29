<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  defaultValue?: string | number;
  modelValue?: string | number;
  class?: HTMLAttributes["class"];
}>();

const emits = defineEmits<{
  (e: "update:modelValue", payload: string | number): void;
}>();

function onInput(event: Event) {
  const target = event.target as HTMLInputElement;
  emits("update:modelValue", target.value);
}
</script>

<template>
  <input
    :value="modelValue ?? defaultValue"
    :class="
      cn(
        'flex h-8 w-full rounded-lg border border-slate-700 bg-slate-900/90 px-3 py-1 text-xs text-slate-100 shadow-sm transition-colors placeholder:text-slate-500 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/40 disabled:cursor-not-allowed disabled:opacity-50 font-normal',
        props.class,
      )
    "
    @input="onInput"
  />
</template>
