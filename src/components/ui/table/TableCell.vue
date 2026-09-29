<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();
</script>

<template>
  <td
    :class="
      cn(
        'p-3.5 align-middle text-slate-200 [&:has([role=checkbox])]:pr-0 text-xs',
        props.class,
      )
    "
  >
    <slot />
  </td>
</template>
