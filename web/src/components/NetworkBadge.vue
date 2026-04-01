<template>
  <div class="flex items-center gap-3">
    <!-- Interface selector -->
    <div class="relative">
      <button
        @click="showDropdown = !showDropdown"
        class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium text-gray-600 dark:text-gray-300
               bg-white/50 dark:bg-white/5 border border-gray-200/70 dark:border-white/10
               hover:bg-white/80 dark:hover:bg-white/10 transition-colors"
      >
        <component :is="ifaceIcon" class="w-3.5 h-3.5" />
        <span>{{ store.selectedInterface || 'auto' }}</span>
        <ChevronDown class="w-3 h-3 opacity-60" />
      </button>

      <!-- Dropdown -->
      <div v-if="showDropdown"
           class="absolute top-full mt-1 right-0 z-50 min-w-[160px] glass-panel rounded-xl border border-gray-200/50 dark:border-white/10 shadow-xl overflow-hidden">
        <button
          v-for="iface in availableInterfaces"
          :key="iface.name"
          @click="selectInterface(iface.name)"
          class="w-full flex items-center gap-2 px-3 py-2 text-xs text-left hover:bg-white/60 dark:hover:bg-gray-700/50 transition-colors"
          :class="store.selectedInterface === iface.name ? 'bg-indigo-50 dark:bg-indigo-900/30 text-indigo-700 dark:text-indigo-300 font-semibold' : 'text-gray-700 dark:text-gray-300'"
        >
          <component :is="getIfaceIcon(iface.name)" class="w-3.5 h-3.5 shrink-0" />
          <span class="truncate">{{ iface.name }}</span>
          <span v-if="iface.connected" class="ml-auto w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0"></span>
        </button>
      </div>
    </div>

    <!-- Rx -->
    <div class="flex items-center gap-1 text-xs font-mono">
      <ArrowDown class="w-3.5 h-3.5 text-emerald-500" />
      <span class="font-semibold text-gray-700 dark:text-gray-200">{{ formatBytes(rxRate) }}/s</span>
    </div>

    <!-- Tx -->
    <div class="flex items-center gap-1 text-xs font-mono">
      <ArrowUp class="w-3.5 h-3.5 text-rose-500" />
      <span class="font-semibold text-gray-700 dark:text-gray-200">{{ formatBytes(txRate) }}/s</span>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { ArrowDown, ArrowUp, ChevronDown, Wifi, Network } from 'lucide-vue-next'
import { useMetricsStore } from '../store.js'
import { formatBytes } from '../utils/format.js'

const store = useMetricsStore()
const showDropdown = ref(false)

const selectedIfaceData = computed(() => {
  if (!store.payload?.network || !store.selectedInterface) return null
  return store.payload.network.find(n => n.name === store.selectedInterface)
})

const rxRate = computed(() => selectedIfaceData.value?.rx_bytes_per_sec ?? 0)
const txRate = computed(() => selectedIfaceData.value?.tx_bytes_per_sec ?? 0)

const availableInterfaces = computed(() => {
  if (!store.payload?.network) return []
  // Sort: LAN first, then WiFi, then others
  return [...store.payload.network].sort((a, b) => {
    const rank = (name) => {
      if (name.startsWith('en') || name.startsWith('eth')) return 0
      if (name.startsWith('wl')) return 1
      return 2
    }
    return rank(a.name) - rank(b.name)
  })
})

const ifaceIcon = computed(() => {
  const name = store.selectedInterface || ''
  if (name.startsWith('wl')) return Wifi
  return Network
})

function getIfaceIcon(name) {
  if (name.startsWith('wl')) return Wifi
  return Network
}

function selectInterface(name) {
  store.selectedInterface = name
  store.saveConfig({ network_interface: name })
  showDropdown.value = false
}

// Close dropdown on outside click
function handleClickOutside(e) {
  if (!e.target.closest('.relative')) showDropdown.value = false
}
onMounted(() => document.addEventListener('click', handleClickOutside))
onBeforeUnmount(() => document.removeEventListener('click', handleClickOutside))


</script>
