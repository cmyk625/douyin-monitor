<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();
</script>

<template>
  <thead :class="cn('[&_tr]:border-b bg-slate-950/95 sticky top-0 z-10 backdrop-blur-md shadow-sm', props.class)">
    <slot />
  </thead>
</template>
