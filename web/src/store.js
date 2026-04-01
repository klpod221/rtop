import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useMetricsStore = defineStore('metrics', () => {
  const isConnected = ref(false)
  const payload = ref(null)
  const webConfig = ref({
    port: 8080,
    network_interface: '',
    storage_filter: []
  })
  const selectedInterface = ref('')

  let ws = null

  function getApiBase() {
    const port = window.location.port === '5173' || window.location.port === '5174' ? '8080' : window.location.port
    const host = window.location.hostname
    const proto = window.location.protocol
    return `${proto}//${host}:${port}`
  }

  async function loadConfig() {
    try {
      const res = await fetch(`${getApiBase()}/api/config`)
      if (res.ok) {
        const data = await res.json()
        webConfig.value = { ...webConfig.value, ...data }
        if (data.network_interface) {
          selectedInterface.value = data.network_interface
        }
      }
    } catch (e) {
      console.warn('Could not load web config:', e)
    }
  }

  async function saveConfig(partial) {
    const updated = { ...webConfig.value, ...partial }
    webConfig.value = updated
    try {
      await fetch(`${getApiBase()}/api/config`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(updated)
      })
    } catch (e) {
      console.warn('Could not save web config:', e)
    }
  }

  function connect() {
    if (ws) ws.close()

    const port = window.location.port === '5173' || window.location.port === '5174' ? '8080' : window.location.port
    const host = window.location.hostname

    const wsProto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${wsProto}//${host}:${port}/ws`

    ws = new WebSocket(wsUrl)

    ws.onopen = () => {
      isConnected.value = true
    }

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        payload.value = data

        // Auto-pick interface on first data if none selected
        if (!selectedInterface.value && data.network && data.network.length > 0) {
          selectedInterface.value = pickBestInterface(data.network)
        }
      } catch (err) {
        console.error('WebSocket receive error:', err)
      }
    }

    ws.onclose = () => {
      isConnected.value = false
      setTimeout(connect, 3000)
    }
  }

  // Priority: LAN (en*) > WiFi (wl*) > first connected > first
  function pickBestInterface(interfaces) {
    const saved = webConfig.value.network_interface
    if (saved && interfaces.find(i => i.name === saved)) return saved

    const lan = interfaces.find(i => i.name.startsWith('en') && i.connected)
    if (lan) return lan.name
    const wifi = interfaces.find(i => i.name.startsWith('wl') && i.connected)
    if (wifi) return wifi.name
    const anyConnected = interfaces.find(i => i.connected)
    if (anyConnected) return anyConnected.name
    return interfaces[0]?.name || ''
  }

  return {
    isConnected,
    payload,
    webConfig,
    selectedInterface,
    connect,
    loadConfig,
    saveConfig,
    pickBestInterface
  }
})
