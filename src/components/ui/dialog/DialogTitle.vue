<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { DialogTitle, type DialogTitleProps, useForwardProps } from "radix-vue";
import { cn } from "@/lib/utils";

const props = defineProps<DialogTitleProps & { class?: HTMLAttributes["class"] }>();
const forwarded = useForwardProps(props);
</script>

<template>
  <DialogTitle
    v-bind="forwarded"
    :class="cn('text-base font-semibold leading-none tracking-tight text-foreground', props.class)"
  >
    <slot />
  </DialogTitle>
</template>
