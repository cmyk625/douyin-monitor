<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();
</script>

<template>
  <th
    :class="
      cn(
        'h-9 px-3.5 text-left align-middle font-semibold text-slate-400 [&:has([role=checkbox])]:pr-0 whitespace-nowrap text-xs',
        props.class,
      )
    "
  >
    <slot />
  </th>
</template>
