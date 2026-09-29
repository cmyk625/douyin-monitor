<script setup lang="ts">
import { computed } from "vue";
import { sparkPath } from "../utils";

const props = withDefaults(
  defineProps<{
    values: Array<number | null>;
    width?: number;
    height?: number;
    label?: string;
    color?: string;
    fillColor?: string;
    showDot?: boolean;
    strokeWidth?: number;
  }>(),
  {
    width: 260,
    height: 56,
    label: "",
    color: "#38bdf8",
    fillColor: "",
    showDot: true,
    strokeWidth: 2,
  },
);

const geom = computed(() => sparkPath(props.values, props.width, props.height));
const last = computed(() => {
  const nums = props.values.filter((v): v is number => v !== null && v !== undefined);
  return nums.length ? nums[nums.length - 1] : null;
});

const uid = Math.random().toString(36).slice(2, 8);
const gradId = `spark-grad-${uid}`;
</script>

<template>
  <svg
    class="spark block w-full h-auto max-w-full overflow-visible"
    :viewBox="`0 0 ${width} ${height}`"
    role="img"
    :aria-label="label"
  >
    <defs>
      <linearGradient :id="gradId" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" :stop-color="color" stop-opacity="0.25" />
        <stop offset="100%" :stop-color="color" stop-opacity="0.01" />
      </linearGradient>
    </defs>
    <path v-if="geom.area" :d="geom.area" :fill="fillColor || `url(#${gradId})`" stroke="none" />
    <path
      v-if="geom.line"
      :d="geom.line"
      fill="none"
      :stroke="color"
      :stroke-width="strokeWidth"
      stroke-linejoin="round"
      stroke-linecap="round"
    />
    <circle
      v-if="showDot && geom.lastPoint"
      :cx="geom.lastPoint[0]"
      :cy="geom.lastPoint[1]"
      r="3.5"
      :fill="color"
    />
    <circle
      v-if="showDot && geom.lastPoint"
      :cx="geom.lastPoint[0]"
      :cy="geom.lastPoint[1]"
      r="1.8"
      fill="#ffffff"
    />
    <title>{{ label }}：{{ last ?? "无数据" }}</title>
  </svg>
</template>
