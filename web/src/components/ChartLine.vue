<template>
  <div class="glass-panel p-5 rounded-2xl border border-gray-200/50 dark:border-white/10 shadow-lg h-full flex flex-col">
    <div class="flex justify-between items-center mb-2 z-10 relative">
      <h3 class="text-sm font-bold text-gray-700 dark:text-gray-300 uppercase tracking-wider">{{ title }}</h3>
    </div>
    <div class="flex-1 w-full relative min-h-[220px]">
      <v-chart class="w-full h-full absolute inset-0" :option="chartOption" autoresize />
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { LineChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  GridComponent,
  DataZoomComponent
} from 'echarts/components'
import VChart from 'vue-echarts'

use([
  CanvasRenderer,
  LineChart,
  TitleComponent,
  TooltipComponent,
  GridComponent,
  DataZoomComponent
])

const props = defineProps({
  title: String,
  data: {
    type: Array,
    default: () => []
  },
  color: {
    type: String,
    default: '#3b82f6'
  },
  unit: {
    type: String,
    default: ''
  }
})

const chartOption = computed(() => {
  const times = props.data.map(d => {
    const dObj = new Date(d.time)
    return `${dObj.getSeconds().toString().padStart(2, '0')}s`
  })
  const vals = props.data.map(d => d.value)

  const textColor = '#9ca3af'
  const splitLineColor = 'rgba(255,255,255,0.05)'
  const isPercent = props.title.includes('%') || props.unit === '%'

  return {
    animation: false,
    tooltip: {
      trigger: 'axis',
      backgroundColor: 'rgba(31, 41, 55, 0.9)',
      borderColor: '#374151',
      textStyle: { color: '#f3f4f6' },
      formatter: (params) => {
        const v = params[0]?.value ?? 0
        return `${isPercent ? v.toFixed(1) + '%' : v.toFixed(2) + (props.unit ? ' ' + props.unit : '')}`
      }
    },
    grid: { left: 40, right: 10, top: 10, bottom: 20 },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: times,
      axisLabel: { color: textColor, showMinLabel: false, showMaxLabel: false },
      axisLine: { show: false },
      axisTick: { show: false }
    },
    yAxis: {
      type: 'value',
      min: 0,
      max: isPercent ? 100 : undefined,
      splitLine: { lineStyle: { color: splitLineColor, type: 'dashed' } },
      axisLabel: { color: textColor, formatter: isPercent ? '{value}%' : '{value}' }
    },
    series: [
      {
        data: vals,
        type: 'line',
        smooth: 0.4,
        symbol: 'none',
        lineStyle: { width: 3, color: props.color },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: props.color + '80' },
              { offset: 1, color: props.color + '00' }
            ]
          }
        }
      }
    ]
  }
})
</script>
