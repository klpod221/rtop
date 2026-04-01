<template>
  <div class="glass-panel p-5 rounded-2xl border border-gray-200/50 dark:border-white/10 shadow-lg">
    <!-- Header -->
    <div class="flex items-center gap-2 mb-4">
      <Cpu class="w-4 h-4 text-indigo-500 font-bold" />
      <h3 class="text-sm font-bold text-gray-700 dark:text-gray-300 uppercase tracking-wider">CPU</h3>
    </div>

    <!-- CPU Name -->
    <div class="text-sm font-semibold text-gray-800 dark:text-gray-100 mb-4 truncate" :title="data.cpu_name">
      {{ data.cpu_name || 'Unknown CPU' }}
    </div>

    <!-- Usage Bar -->
    <div class="space-y-3">
      <div class="space-y-1">
        <div class="flex justify-between text-xs">
          <span class="text-gray-500 dark:text-gray-400 font-medium">Usage</span>
          <span class="font-bold text-gray-700 dark:text-gray-200">{{ data.usage_percent?.toFixed(1) || 0 }}%</span>
        </div>
        <div class="w-full bg-gray-100 dark:bg-gray-800/80 rounded-full h-2 overflow-hidden">
          <div class="h-2 rounded-full transition-all duration-300 bg-gradient-to-r from-indigo-400 to-purple-500"
               :style="{ width: (data.usage_percent || 0) + '%' }"></div>
        </div>
      </div>

      <!-- Stats grid -->
      <div class="grid grid-cols-2 gap-2 mt-3">
        <div class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Freq</span>
          <span class="text-xs font-bold ml-auto text-indigo-500 dark:text-indigo-400">
            {{ data.freq_mhz && data.freq_mhz.length > 0 ? Math.round(data.freq_mhz[0]) : 0 }} MHz
          </span>
        </div>
        <div v-if="data.package_temp_c > 0" class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Temp</span>
          <span class="text-xs font-bold ml-auto" :class="data.package_temp_c > 85 ? 'text-red-500' : 'text-gray-700 dark:text-gray-200'">
            {{ data.package_temp_c }}°C
          </span>
        </div>
        <div v-if="data.power_watts > 0" class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Power</span>
          <span class="text-xs font-bold ml-auto text-orange-500">{{ data.power_watts.toFixed(1) }}W</span>
        </div>
        <div v-if="data.load_avg && data.load_avg.length" class="flex items-center gap-1.5 px-3 py-2 bg-white/40 dark:bg-gray-800/40 rounded-xl">
          <span class="text-xs text-gray-500">Load</span>
          <span class="text-xs font-bold ml-auto text-blue-500 dark:text-blue-400">{{ data.load_avg[0].toFixed(2) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { Cpu } from 'lucide-vue-next'

defineProps({
  data: {
    type: Object,
    required: true
  }
})
</script>
