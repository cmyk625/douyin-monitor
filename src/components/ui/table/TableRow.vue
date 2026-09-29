<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();
</script>

<template>
  <tr
    :class="
      cn(
        'border-b border-slate-800/70 transition-colors hover:bg-slate-800/40 data-[state=selected]:bg-slate-800',
        props.class,
      )
    "
  >
    <slot />
  </tr>
</template>
