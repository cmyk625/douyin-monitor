<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = withDefaults(
  defineProps<{
    class?: HTMLAttributes["class"];
    colspan?: number;
  }>(),
  {
    colspan: 1,
    class: undefined,
  },
);
</script>

<template>
  <tr class="hover:bg-transparent">
    <td
      :class="
        cn(
          'p-8 text-center text-xs text-muted-foreground whitespace-nowrap',
          props.class,
        )
      "
      :colspan="colspan"
    >
      <slot />
    </td>
  </tr>
</template>
