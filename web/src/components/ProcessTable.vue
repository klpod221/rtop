<template>
  <div class="glass-panel rounded-2xl overflow-hidden shadow-lg border border-gray-200/50 dark:border-white/10 flex flex-col h-full max-h-[640px]">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-gray-100 dark:border-gray-800/50 bg-white/40 dark:bg-gray-900/40 backdrop-blur-sm shrink-0">
      <div class="flex items-center gap-2 mb-3">
        <Cpu class="w-4 h-4 text-indigo-500" />
        <h3 class="text-sm font-bold text-gray-700 dark:text-gray-300 uppercase tracking-wider">Processes</h3>
        <span class="ml-auto text-xs text-gray-400">{{ filteredProcesses.length }} shown</span>
      </div>

      <!-- Search & Filter bar -->
      <div class="flex gap-2">
        <!-- Search -->
        <div class="relative flex-1">
          <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Search name, cmdline..."
            class="w-full pl-8 pr-3 py-1.5 text-xs rounded-lg bg-white/60 dark:bg-gray-800/60 border border-gray-200/60 dark:border-gray-700/60
                   text-gray-700 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-500
                   focus:outline-none focus:ring-1 focus:ring-indigo-500/50 transition"
          />
        </div>

        <!-- User filter -->
        <select
          v-model="userFilter"
          class="px-2 py-1.5 text-xs rounded-lg bg-white/60 dark:bg-gray-800/60 border border-gray-200/60 dark:border-gray-700/60
                 text-gray-700 dark:text-gray-200 focus:outline-none focus:ring-1 focus:ring-indigo-500/50 transition"
        >
          <option value="">All users</option>
          <option v-for="u in uniqueUsers" :key="u" :value="u">{{ u }}</option>
        </select>
      </div>
    </div>

    <!-- Table -->
    <div class="overflow-auto flex-1">
      <table class="w-full text-left text-sm text-gray-600 dark:text-gray-400 border-collapse">
        <thead class="sticky top-0 z-10 text-xs uppercase bg-gray-100/90 dark:bg-gray-800/90 backdrop-blur-md text-gray-500 dark:text-gray-500 font-semibold tracking-wider shadow-sm">
          <tr>
            <th class="px-4 py-3 cursor-pointer select-none hover:text-gray-700 dark:hover:text-gray-300 transition-colors" @click="setSort('pid')">
              <div class="flex items-center gap-1">
                PID
                <SortIcon field="pid" />
              </div>
            </th>
            <th class="px-4 py-3 cursor-pointer select-none hover:text-gray-700 dark:hover:text-gray-300 transition-colors" @click="setSort('name')">
              <div class="flex items-center gap-1">
                Name
                <SortIcon field="name" />
              </div>
            </th>
            <th class="px-4 py-3 text-right cursor-pointer select-none hover:text-gray-700 dark:hover:text-gray-300 transition-colors" @click="setSort('cpu')">
              <div class="flex items-center justify-end gap-1">
                CPU %
                <SortIcon field="cpu" />
              </div>
            </th>
            <th class="px-4 py-3 text-right cursor-pointer select-none hover:text-gray-700 dark:hover:text-gray-300 transition-colors" @click="setSort('mem')">
              <div class="flex items-center justify-end gap-1">
                MEM
                <SortIcon field="mem" />
              </div>
            </th>
            <th class="px-4 py-3 text-right">USER</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-100 dark:divide-gray-800/50 bg-white/20 dark:bg-transparent">
          <tr v-for="proc in filteredProcesses" :key="proc.pid"
              class="hover:bg-white/60 dark:hover:bg-gray-800/40 transition-colors duration-150">
            <td class="px-4 py-2.5 font-mono text-xs text-gray-400 dark:text-gray-500">{{ proc.pid }}</td>
            <td class="px-4 py-2.5 font-medium text-gray-800 dark:text-gray-200 truncate max-w-[200px]" :title="proc.cmdline">
              <span v-html="highlight(proc.name, searchQuery)"></span>
            </td>
            <td class="px-4 py-2.5 text-right">
              <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold"
                    :class="proc.cpu_percent > 20
                      ? 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400'
                      : proc.cpu_percent > 5
                      ? 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400'
                      : 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'">
                {{ proc.cpu_percent.toFixed(1) }}%
              </span>
            </td>
            <td class="px-4 py-2.5 text-right font-medium text-xs">{{ formatBytes(proc.mem_rss_bytes) }}</td>
            <td class="px-4 py-2.5 text-right text-xs opacity-70">{{ proc.user }}</td>
          </tr>
          <tr v-if="filteredProcesses.length === 0">
            <td colspan="5" class="px-6 py-12 text-center text-gray-500 text-sm">
              {{ processes && processes.length > 0 ? 'No processes match your filter.' : 'Waiting for process data...' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, h } from 'vue'
import { Search, Cpu, ChevronUp, ChevronDown } from 'lucide-vue-next'
import { formatBytes } from '../utils/format.js'

const props = defineProps({
  processes: { type: Array, default: () => [] }
})

const searchQuery = ref('')
const userFilter = ref('')
const sortField = ref('cpu')
const sortDir = ref('desc') // 'asc' | 'desc'

const uniqueUsers = computed(() => {
  const users = new Set(props.processes.map(p => p.user).filter(Boolean))
  return [...users].sort()
})

function setSort(field) {
  if (sortField.value === field) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortField.value = field
    sortDir.value = field === 'name' || field === 'pid' ? 'asc' : 'desc'
  }
}

const filteredProcesses = computed(() => {
  let list = props.processes || []

  // Search filter
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase()
    list = list.filter(p =>
      p.name?.toLowerCase().includes(q) ||
      p.cmdline?.toLowerCase().includes(q)
    )
  }

  // User filter
  if (userFilter.value) {
    list = list.filter(p => p.user === userFilter.value)
  }

  // Sort
  list = [...list].sort((a, b) => {
    let va, vb
    switch (sortField.value) {
      case 'pid': va = a.pid; vb = b.pid; break
      case 'name': va = a.name?.toLowerCase(); vb = b.name?.toLowerCase(); break
      case 'mem': va = a.mem_rss_bytes; vb = b.mem_rss_bytes; break
      case 'cpu':
      default: va = a.cpu_percent; vb = b.cpu_percent; break
    }
    if (va < vb) return sortDir.value === 'asc' ? -1 : 1
    if (va > vb) return sortDir.value === 'asc' ? 1 : -1
    return 0
  })

  return list
})

function highlight(text, query) {
  if (!query.trim()) return text || ''
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return (text || '').replace(new RegExp(`(${escaped})`, 'gi'),
    '<mark class="bg-yellow-200 dark:bg-yellow-800/60 rounded px-0.5">$1</mark>')
}

// SortIcon as inline component
const SortIcon = {
  props: ['field'],
  setup(p) {
    return () => {
      if (sortField.value !== p.field) return null
      return sortDir.value === 'asc'
        ? h(ChevronUp, { class: 'w-3 h-3 text-indigo-500' })
        : h(ChevronDown, { class: 'w-3 h-3 text-indigo-500' })
    }
  }
}

</script>
