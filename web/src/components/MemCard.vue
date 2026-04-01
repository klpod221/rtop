<template>
  <div
    class="glass-panel p-5 rounded-2xl border border-gray-200/50 dark:border-white/10 shadow-lg flex flex-col justify-between"
  >
    <div>
      <!-- Header -->
      <div class="flex items-center gap-2 mb-4">
        <MemoryStick class="w-4 h-4 text-purple-500 font-bold" />
        <h3
          class="text-sm font-bold text-gray-700 dark:text-gray-300 uppercase tracking-wider"
        >
          Memory
        </h3>
      </div>

      <!-- physical_ram Info -->
      <div
        class="text-sm font-semibold text-gray-800 dark:text-gray-100 mb-4 truncate text-wrap"
        :title="ramInfo"
        v-html="ramInfo || 'Unknown RAM'"
      ></div>
    </div>

    <!-- Usage Bar -->
    <div class="space-y-3 mt-auto">
      <div class="space-y-1">
        <div class="flex justify-between text-xs">
          <span class="text-gray-500 dark:text-gray-400 font-medium"
            >RAM Usage</span
          >
          <span class="font-bold text-gray-700 dark:text-gray-200">
            {{ formatBytes(data.used) }} ({{ percent.toFixed(1) }}%)
          </span>
        </div>
        <div
          class="w-full bg-gray-100 dark:bg-gray-800/80 rounded-full h-2 overflow-hidden shadow-inner"
        >
          <div
            class="h-2 rounded-full transition-all duration-300 bg-linear-to-r from-purple-400 to-pink-500"
            :style="{ width: percent + '%' }"
          ></div>
        </div>
      </div>

      <!-- Swap Bar -->
      <div v-if="data.swap_total > 0" class="space-y-1 pt-1">
        <div class="flex justify-between text-xs">
          <span class="text-gray-500 dark:text-gray-400 font-medium">Swap</span>
          <span class="font-bold text-gray-700 dark:text-gray-200">
            {{ formatBytes(data.swap_used) }} ({{ swapPercent.toFixed(1) }}%)
          </span>
        </div>
        <div
          class="w-full bg-gray-100 dark:bg-gray-800/80 rounded-full h-1.5 overflow-hidden shadow-inner"
        >
          <div
            class="h-1.5 rounded-full bg-linear-to-r from-gray-400 to-gray-500"
            :style="{ width: swapPercent + '%' }"
          ></div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { formatBytes } from "../utils/format.js";
import { MemoryStick } from "lucide-vue-next";

const props = defineProps({
  data: {
    type: Object,
    required: true,
  },
  host: {
    type: Object,
    default: () => ({}),
  },
});

const ramInfo = computed(() => {
  if (props.host?.physical_ram?.length > 0) {
    return props.host.physical_ram.join(" <br /> ");
  }
  return formatBytes(props.data.total);
});

const percent = computed(() => {
  if (!props.data || !props.data.total) return 0;
  return (props.data.used / props.data.total) * 100;
});

const swapPercent = computed(() => {
  if (!props.data || !props.data.swap_total) return 0;
  return (props.data.swap_used / props.data.swap_total) * 100;
});
</script>
