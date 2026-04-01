<template>
  <div
    class="min-h-screen relative font-sans selection:bg-indigo-500/30 flex flex-col"
  >
    <!-- Background elements for modern glass effect -->
    <div class="fixed inset-0 z-[-1] overflow-hidden pointer-events-none">
      <div
        class="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] rounded-full bg-indigo-500/10 dark:bg-indigo-500/20 blur-[100px] mix-blend-multiply dark:mix-blend-screen transition-all duration-1000"
      ></div>
      <div
        class="absolute top-[20%] right-[-10%] w-[50%] h-[50%] rounded-full bg-purple-500/10 dark:bg-purple-600/20 blur-[120px] mix-blend-multiply dark:mix-blend-screen transition-all duration-1000"
      ></div>
      <div
        class="absolute bottom-[-10%] left-[20%] w-[60%] h-[60%] rounded-full bg-blue-500/10 dark:bg-blue-600/20 blur-[120px] mix-blend-multiply dark:mix-blend-screen transition-all duration-1000"
      ></div>
    </div>

    <!-- Header -->
    <header
      class="sticky top-0 z-50 glass-panel border-b border-gray-200/50 dark:border-white/10 rounded-none shadow-sm"
    >
      <div
        class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between"
      >
        <!-- Left: Logo + status -->
        <div class="flex items-center gap-3">
          <div
            class="w-8 h-8 rounded-lg bg-linear-to-br from-indigo-500 to-purple-600 flex items-center justify-center shadow-lg shadow-indigo-500/30"
          >
            <Zap class="w-4 h-4 text-white" />
          </div>
          <h1
            class="text-xl font-bold tracking-tight bg-clip-text text-transparent bg-linear-to-r from-gray-900 to-gray-600 dark:from-white dark:to-gray-300"
          >
            rtop
          </h1>
          <span
            class="px-2.5 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider transition-colors duration-300 ml-1"
            :class="
              store.isConnected
                ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-500/20 dark:text-emerald-400'
                : 'bg-rose-100 text-rose-800 dark:bg-rose-500/20 dark:text-rose-400'
            "
          >
            {{ store.isConnected ? "Connected" : "Disconnected" }}
          </span>
        </div>

        <!-- Right: Network badge + host info -->
        <div class="flex items-center gap-4">
          <!-- Network topbar badge -->
          <NetworkBadge v-if="payload?.network" />

          <!-- Divider -->
          <div
            class="w-px h-6 bg-gray-200 dark:bg-white/10 hidden md:block"
          ></div>

          <!-- Host info -->
          <div v-if="payload?.host" class="hidden md:flex flex-col items-end">
            <span
              class="text-sm font-semibold text-gray-800 dark:text-gray-200"
              >{{ payload.host.os_vendor }}</span
            >
            <span
              class="text-xs text-gray-500 dark:text-gray-400 font-medium opacity-80"
            >
              {{ payload.host.kernel_version }}
            </span>
          </div>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main
      class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 space-y-6 flex-1 w-full"
    >
      <div
        v-if="!payload"
        class="flex flex-col items-center justify-center h-64 gap-5"
      >
        <div
          class="w-14 h-14 border-4 border-indigo-500/20 border-t-indigo-500 rounded-full animate-spin shadow-lg shadow-indigo-500/20"
        ></div>
        <p class="text-gray-500 dark:text-gray-400 font-medium animate-pulse">
          Waiting for telemetry data...
        </p>
      </div>

      <template v-else>
        <!-- Top Metrics: CPU + GPU cards Memory -->
        <div
          class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-[repeat(auto-fit,minmax(280px,1fr))] gap-6"
        >
          <CpuCard v-if="payload.cpu" :data="payload.cpu" />

          <!-- GPU cards -->
          <GpuCard
            v-if="payload.intel_gpu"
            type="intel"
            :data="payload.intel_gpu"
          />
          <GpuCard
            v-for="gpu in payload.nvidia_gpus || []"
            :key="gpu.Name"
            type="nvidia"
            :data="gpu"
          />
          <GpuCard
            v-for="gpu in payload.amd_gpus || []"
            :key="gpu.Name"
            type="amd"
            :data="gpu"
          />

          <MemCard
            v-if="payload.memory"
            :data="payload.memory"
            :host="payload.host"
          />
        </div>

        <!-- Charts Row -->
        <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
          <div class="h-[320px]">
            <ChartLine
              title="CPU History (%)"
              :data="cpuHistory"
              color="#6366f1"
              unit="%"
            />
          </div>
          <div class="h-[320px]">
            <ChartLine
              title="Memory History (%)"
              :data="memHistory"
              color="#a855f7"
              unit="%"
            />
          </div>
        </div>

        <!-- Bottom Row: Storage & Processes -->
        <div class="grid grid-cols-1 xl:grid-cols-3 gap-6">
          <div class="xl:col-span-1 flex flex-col">
            <StoragePanel :disks="payload.disks_space" :io="payload.disks_io" />
          </div>

          <div class="xl:col-span-2">
            <ProcessTable :processes="payload.processes" />
          </div>
        </div>
      </template>
    </main>

    <!-- Footer -->
    <footer
      class="mt-auto py-6 text-center text-xs font-medium text-gray-500 dark:text-gray-400"
    >
      Made with <span class="text-rose-500 mx-0.5">❤️</span> by
      <a
        href="https://klpod221.com"
        target="_blank"
        rel="noopener noreferrer"
        class="text-indigo-500 dark:text-indigo-400 hover:text-indigo-600 dark:hover:text-indigo-300 transition-colors font-bold"
        >klpod221</a
      >
    </footer>
  </div>
</template>

<script setup>
import { computed, watch, ref, onMounted } from "vue";
import { Zap } from "lucide-vue-next";
import { useMetricsStore } from "./store.js";
import CpuCard from "./components/CpuCard.vue";
import MemCard from "./components/MemCard.vue";
import ProcessTable from "./components/ProcessTable.vue";
import ChartLine from "./components/ChartLine.vue";
import GpuCard from "./components/GpuCard.vue";
import NetworkBadge from "./components/NetworkBadge.vue";
import StoragePanel from "./components/StoragePanel.vue";

const store = useMetricsStore();
const payload = computed(() => store.payload);

// History data for charts
const MAX_HISTORY = 60;
const cpuHistory = ref([]);
const memHistory = ref([]);

const memPercent = computed(() => {
  if (!payload.value?.memory) return 0;
  return (payload.value.memory.used / payload.value.memory.total) * 100;
});

watch(
  () => store.payload,
  (newPayload) => {
    if (!newPayload) return;
    const now = Date.now();

    // Update CPU history
    if (newPayload.cpu) {
      cpuHistory.value.push({ time: now, value: newPayload.cpu.usage_percent });
      if (cpuHistory.value.length > MAX_HISTORY) cpuHistory.value.shift();
    }

    // Update Memory history (% instead of GB)
    if (newPayload.memory) {
      const pct = (newPayload.memory.used / newPayload.memory.total) * 100;
      memHistory.value.push({ time: now, value: pct });
      if (memHistory.value.length > MAX_HISTORY) memHistory.value.shift();
    }
  },
  { deep: true },
);

onMounted(async () => {
  await store.loadConfig();
  store.connect();
});

function formatBytes(bytes) {
  if (!bytes || bytes === 0 || isNaN(bytes)) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  if (i < 0) return "0 B";
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}
</script>
