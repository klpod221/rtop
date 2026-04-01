<template>
  <div class="glass-panel p-5 rounded-2xl border border-gray-200/50 dark:border-white/10 shadow-lg">
    <!-- Header -->
    <div class="flex items-center gap-2 mb-4">
      <component :is="gpuIcon" class="w-4 h-4" :class="iconColor" />
      <h3 class="text-sm font-bold text-gray-700 dark:text-gray-300 uppercase tracking-wider">{{ label }}</h3>
    </div>

    <!-- GPU Name -->
    <div class="text-sm font-semibold text-gray-800 dark:text-gray-100 mb-4 truncate" :title="gpuName">
      {{ gpuName }}
    </div>

    <!-- Usage Bar -->
    <div class="space-y-3">
      <div class="space-y-1">
        <div class="flex justify-between text-xs">
          <span class="text-gray-500 dark:text-gray-400 font-medium">GPU Usage</span>
          <span class="font-bold text-gray-700 dark:text-gray-200">{{ utilizationGPU }}%</span>
        </div>
        <div class="w-full bg-gray-100 dark:bg-gray-800/80 rounded-full h-2 overflow-hidden">
          <div class="h-2 rounded-full transition-all duration-300" :class="barColor"
               :style="{ width: utilizationGPU + '%' }"></div>
        </div>
      </div>

      <!-- VRAM -->
      <div v-if="vramTotal > 0" class="space-y-1">
        <div class="flex justify-between text-xs">
          <span class="text-gray-500 dark:text-gray-400 font-medium">VRAM</span>
          <span class="font-bold text-gray-700 dark:text-gray-200">
            {{ formatBytes(vramUsed) }} / {{ formatBytes(vramTotal) }}
          </span>
        </div>
        <div class="w-full bg-gray-100 dark:bg-gray-800/80 rounded-full h-2 overflow-hidden">
          <div class="h-2 rounded-full transition-all duration-300 bg-gradient-to-r from-blue-400 to-indigo-500"
               :style="{ width: (vramTotal > 0 ? (vramUsed / vramTotal) * 100 : 0) + '%' }"></div>
        </div>
      </div>

      <!-- Stats grid -->
      <div class="grid grid-cols-2 gap-2 mt-3">
        <div v-if="tempC > 0" class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Temp</span>
          <span class="text-xs font-bold ml-auto" :class="tempC > 80 ? 'text-red-500' : 'text-gray-700 dark:text-gray-200'">
            {{ tempC }}°C
          </span>
        </div>
        <div v-if="powerWatts > 0" class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Power</span>
          <span class="text-xs font-bold ml-auto text-orange-500">{{ formatPower(powerWatts) }}</span>
        </div>
        <div v-if="clockCoreMHz > 0" class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Core</span>
          <span class="text-xs font-bold ml-auto text-indigo-500 dark:text-indigo-400">{{ formatMHz(clockCoreMHz) }}</span>
        </div>
        <div v-if="clockMemMHz > 0" class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Mem Clk</span>
          <span class="text-xs font-bold ml-auto text-blue-500 dark:text-blue-400">{{ formatMHz(clockMemMHz) }}</span>
        </div>
      </div>

      <!-- Intel Engines -->
      <div v-if="type === 'intel' && data.engines && data.engines.length" class="space-y-2 pt-1">
        <div v-for="engine in data.engines" :key="engine.name" class="space-y-1">
          <div class="flex justify-between text-xs">
            <span class="text-gray-600 dark:text-gray-400">{{ engine.name }}</span>
            <span class="font-bold text-gray-700 dark:text-gray-200">{{ (engine.busy_pct || 0).toFixed(1) }}%</span>
          </div>
          <div class="w-full bg-gray-100 dark:bg-gray-800/80 rounded-full h-1.5 overflow-hidden">
            <div class="h-1.5 rounded-full bg-gradient-to-r from-indigo-400 to-purple-500"
                 :style="{ width: (engine.busy_pct || 0) + '%' }"></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { Zap, Layers, Flame } from 'lucide-vue-next'
import { formatBytes, formatMHz, formatPower } from '../utils/format.js'

const props = defineProps({
  type: {
    type: String, // 'intel' | 'nvidia' | 'amd'
    required: true
  },
  data: {
    type: Object,
    required: true
  }
})

const gpuIcon = computed(() => {
  if (props.type === 'intel') return Zap
  if (props.type === 'nvidia') return Layers
  return Flame
})

const iconColor = computed(() => {
  if (props.type === 'intel') return 'text-blue-500'
  if (props.type === 'nvidia') return 'text-green-500'
  return 'text-red-500'
})

const barColor = computed(() => {
  if (props.type === 'intel') return 'bg-gradient-to-r from-blue-400 to-indigo-500'
  if (props.type === 'nvidia') return 'bg-gradient-to-r from-green-400 to-emerald-500'
  return 'bg-gradient-to-r from-red-400 to-orange-500'
})

const label = computed(() => {
  if (props.type === 'intel') return 'Intel GPU'
  if (props.type === 'nvidia') return 'NVIDIA GPU'
  return 'AMD GPU'
})

// Nvidia & AMD use PascalCase (no JSON tags in Go struct)
// Intel uses snake_case (has JSON tags in Go struct)
const gpuName = computed(() => {
  if (props.type === 'intel') return 'Intel GPU (PMU)'
  return props.data.Name || 'Unknown'
})

const utilizationGPU = computed(() => {
  if (props.type === 'intel') {
    // Intel: use primary engine busy_pct or 0
    const render = props.data.engines?.find(e => e.name?.toLowerCase().includes('render'))
    return Math.round(render?.busy_pct || 0)
  }
  return props.data.UtilizationGPU || 0
})

const vramTotal = computed(() => {
  if (props.type === 'intel') return 0
  return props.data.VRAMTotal || 0
})

const vramUsed = computed(() => {
  if (props.type === 'intel') return 0
  return props.data.VRAMUsed || 0
})

const tempC = computed(() => {
  if (props.type === 'intel') return 0
  return props.data.TempC || 0
})

const powerWatts = computed(() => {
  if (props.type === 'intel') return props.data.power_gpu_watts || 0
  return props.data.PowerWatts || 0
})

const clockCoreMHz = computed(() => {
  if (props.type === 'intel') return props.data.freq_act_mhz || 0
  return props.data.ClockCoreMHz || 0
})

const clockMemMHz = computed(() => {
  if (props.type === 'intel') return 0
  return props.data.ClockMemMHz || 0
})


</script>
