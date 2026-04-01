<template>
  <div class="glass-panel p-6 rounded-2xl border border-gray-200/50 dark:border-white/10 shadow-lg flex-1 flex flex-col">
    <!-- Header with Tabs and Filter Toggle -->
    <div class="flex items-center justify-between mb-5">
      <div class="flex items-center gap-4">
        <div class="flex items-center gap-2 text-teal-500">
          <HardDrive class="w-4 h-4" />
          <h3 class="text-sm font-bold uppercase tracking-wider hidden sm:block">Storage</h3>
        </div>
        
        <!-- Tabs -->
        <div class="flex bg-gray-100/50 dark:bg-gray-800/50 rounded-lg p-0.5 border border-gray-200/50 dark:border-gray-700/50">
          <button
            @click="activeTab = 'mounts'"
            class="px-3 py-1 text-xs font-semibold rounded-md transition-colors"
            :class="activeTab === 'mounts' ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm' : 'text-gray-500 hover:text-gray-700 dark:text-gray-400'"
          >
            Mounts
          </button>
          <button
            @click="activeTab = 'io'"
            class="px-3 py-1 text-xs font-semibold rounded-md transition-colors"
            :class="activeTab === 'io' ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm' : 'text-gray-500 hover:text-gray-700 dark:text-gray-400'"
          >
            I/O Details
          </button>
        </div>
      </div>
      
      <button
        v-if="activeTab === 'mounts'"
        @click="showFilter = !showFilter"
        class="flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-medium transition-colors"
        :class="showFilter
          ? 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/40 dark:text-indigo-300'
          : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800/50'"
      >
        <SlidersHorizontal class="w-3.5 h-3.5" />
        <span>Filter</span>
      </button>
    </div>

    <!-- Filter panel (only for Mounts) -->
    <div v-if="activeTab === 'mounts' && showFilter" class="mb-4 p-3 bg-gray-50 dark:bg-gray-800/50 rounded-xl border border-gray-200/50 dark:border-gray-700/50">
      <p class="text-xs text-gray-500 mb-2 font-medium">Show only selected:</p>
      <div class="space-y-1.5 max-h-40 overflow-y-auto">
        <label
          v-for="disk in allDisks"
          :key="disk.mount_point"
          class="flex items-center gap-2 cursor-pointer group"
        >
          <input
            type="checkbox"
            :value="disk.mount_point"
            v-model="localFilter"
            @change="onFilterChange"
            class="w-3.5 h-3.5 rounded accent-indigo-500"
          />
          <span class="text-xs text-gray-600 dark:text-gray-300 group-hover:text-gray-900 dark:group-hover:text-gray-100">
            {{ disk.mount_point }}
            <span class="text-gray-400 dark:text-gray-500 ml-1">({{ disk.fs_type }})</span>
          </span>
        </label>
      </div>
      <button
        v-if="localFilter.length > 0"
        @click="clearFilter"
        class="mt-2 text-xs text-indigo-500 hover:text-indigo-700 dark:hover:text-indigo-300 font-medium"
      >
        Clear all
      </button>
    </div>

    <!-- Scrollable content area -->
    <div class="flex-1 overflow-y-auto pr-1 -mr-1">
      <!-- Mounts Tab -->
      <div v-if="activeTab === 'mounts'" class="space-y-4">
        <div v-for="disk in filteredDisks" :key="disk.mount_point" class="space-y-1.5">
          <div class="flex justify-between text-sm items-end">
            <div class="flex items-center gap-1.5">
              <span class="text-gray-700 dark:text-gray-300 font-semibold truncate max-w-[120px]" :title="disk.mount_point">
                {{ disk.mount_point }}
              </span>
              <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-400 shrink-0">
                {{ disk.fs_type }}
              </span>
            </div>
            
            <div class="flex items-end gap-3 shrink-0">
              <!-- IO Rate Display -->
              <div v-if="disk.ioRate" class="hidden sm:flex items-center gap-2 text-[10px] font-mono mr-1">
                <span class="text-emerald-500 flex items-center"><ArrowDown class="w-3 h-3"/>{{ formatBytes(disk.ioRate.read) }}/s</span>
                <span class="text-rose-500 flex items-center"><ArrowUp class="w-3 h-3"/>{{ formatBytes(disk.ioRate.write) }}/s</span>
              </div>
              
              <span class="text-xs font-medium text-gray-500 dark:text-gray-400 text-right">
                {{ formatBytes(disk.used) }} / {{ formatBytes(disk.total) }}
              </span>
            </div>
          </div>
          <div class="w-full bg-gray-100 dark:bg-gray-800/80 rounded-full h-2 overflow-hidden shadow-inner border border-gray-200/50 dark:border-gray-700/50">
            <div
              class="h-2 rounded-full transition-all duration-500 ease-out"
              :class="disk.used_percent > 85 ? 'bg-gradient-to-r from-red-400 to-rose-500'
                      : disk.used_percent > 60 ? 'bg-gradient-to-r from-amber-400 to-orange-500'
                      : 'bg-gradient-to-r from-teal-400 to-emerald-500'"
              :style="{ width: disk.used_percent + '%' }"
            ></div>
          </div>
        </div>
        <div v-if="filteredDisks.length === 0" class="text-center text-sm text-gray-400 py-6">
          No mounts to show. Adjust filter above.
        </div>
      </div>

      <!-- I/O Tab (Visual mini-bars) -->
      <div v-if="activeTab === 'io'" class="space-y-3">
        <div v-for="disk in ioRatesList" :key="disk.device" class="bg-white/40 dark:bg-gray-800/40 p-3 rounded-xl border border-gray-100 dark:border-gray-700/50">
          <div class="flex items-center justify-between mb-3">
            <div class="font-bold text-gray-700 dark:text-gray-200 text-sm flex items-center gap-1.5">
              <HardDrive class="w-3.5 h-3.5 text-gray-400" />
              {{ disk.device }}
            </div>
            <div class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-gray-200/50 dark:bg-gray-700 text-gray-500 dark:text-gray-400 truncate max-w-[120px]" :title="disk.model">
              {{ disk.model || 'Unknown' }}
            </div>
          </div>
          
          <div class="space-y-2.5">
            <!-- Read Bar -->
            <div>
              <div class="flex items-center justify-between text-xs mb-1">
                <span class="text-gray-500 flex items-center gap-1"><ArrowDown class="w-3 h-3 text-emerald-500"/>Read Rate</span>
                <span class="font-mono font-medium text-emerald-600 dark:text-emerald-400">{{ formatBytes(disk.read_bytes_per_sec) }}/s</span>
              </div>
              <div class="w-full bg-emerald-50 dark:bg-emerald-900/20 rounded-full h-1.5 overflow-hidden">
                <div class="h-1.5 rounded-full bg-emerald-400 transition-all duration-300" :style="{ width: Math.min(100, (disk.read_bytes_per_sec / 100000000) * 100) + '%' }"></div>
              </div>
            </div>
            <!-- Write Bar -->
            <div>
              <div class="flex items-center justify-between text-xs mb-1">
                <span class="text-gray-500 flex items-center gap-1"><ArrowUp class="w-3 h-3 text-rose-500"/>Write Rate</span>
                <span class="font-mono font-medium text-rose-600 dark:text-rose-400">{{ formatBytes(disk.write_bytes_per_sec) }}/s</span>
              </div>
              <div class="w-full bg-rose-50 dark:bg-rose-900/20 rounded-full h-1.5 overflow-hidden">
                <div class="h-1.5 rounded-full bg-rose-400 transition-all duration-300" :style="{ width: Math.min(100, (disk.write_bytes_per_sec / 100000000) * 100) + '%' }"></div>
              </div>
            </div>
          </div>
        </div>
        <div v-if="!ioRatesList || ioRatesList.length === 0" class="text-center text-sm text-gray-400 py-6">
          No disk I/O metrics available.
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { HardDrive, SlidersHorizontal, ArrowDown, ArrowUp } from 'lucide-vue-next'
import { formatBytes } from '../utils/format.js'
import { useMetricsStore } from '../store.js'

const props = defineProps({
  disks: { type: Array, default: () => [] },
  io: { type: Array, default: () => [] }
})

const store = useMetricsStore()
const activeTab = ref('mounts') // 'mounts' | 'io'
const showFilter = ref(false)
const localFilter = ref([])

// IO Rate Tracking
const ioRatesList = computed(() => {
  if (!props.io) return []
  return [...props.io].sort((a, b) => 
    (b.read_bytes_per_sec + b.write_bytes_per_sec) - (a.read_bytes_per_sec + a.write_bytes_per_sec)
  )
})

// Helper to match e.g. /dev/nvme0n1p2 -> nvme0n1
function getIORateForMount(mountDeviceStr) {
  if (!mountDeviceStr || !props.io) return null
  const m = mountDeviceStr.match(/\/dev\/(nvme\d+n\d+|mmcblk\d+|vd[a-z]|sd[a-z]|hd[a-z])/)
  if (!m) return null
  const baseDev = m[1]
  const data = props.io.find(d => d.device === baseDev)
  if (!data) return null
  return { read: data.read_bytes_per_sec, write: data.write_bytes_per_sec }
}

// Initialize filter from stored config
watch(() => store.webConfig.storage_filter, (val) => {
  if (val && val.length > 0) localFilter.value = [...val]
}, { immediate: true })

const allDisks = computed(() => props.disks || [])

const filteredDisks = computed(() => {
  let list = allDisks.value
  if (localFilter.value.length > 0) {
    list = list.filter(d => localFilter.value.includes(d.mount_point))
  }
  return list.map(disk => ({
    ...disk,
    ioRate: getIORateForMount(disk.device)
  }))
})

function onFilterChange() {
  store.saveConfig({ storage_filter: [...localFilter.value] })
}

function clearFilter() {
  localFilter.value = []
  store.saveConfig({ storage_filter: [] })
}

</script>
