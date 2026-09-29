<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
  containerClass?: HTMLAttributes["class"];
}>();
</script>

<template>
  <div :class="cn('relative w-full overflow-auto rounded-xl border border-slate-800 bg-slate-900/80 shadow-sm backdrop-blur-md', props.containerClass)">
    <table :class="cn('w-full caption-bottom text-xs', props.class)">
      <slot />
    </table>
  </div>
</template>
