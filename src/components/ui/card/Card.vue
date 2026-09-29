<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();
</script>

<template>
  <div
    :class="
      cn(
        'rounded-xl border border-slate-800/80 bg-slate-900/80 text-slate-100 shadow-sm backdrop-blur-md transition-all',
        props.class,
      )
    "
  >
    <slot />
  </div>
</template>
