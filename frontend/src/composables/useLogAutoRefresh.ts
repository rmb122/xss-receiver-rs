import { onUnmounted, ref } from 'vue'

const LOG_AUTO_REFRESH_INTERVAL_MS = 6000

export function useLogAutoRefresh(refreshLogs: () => Promise<void>) {
  const autoRefresh = ref(true)
  let refreshTimer: ReturnType<typeof setInterval> | undefined

  function stopAutoRefresh() {
    if (refreshTimer !== undefined) {
      clearInterval(refreshTimer)
      refreshTimer = undefined
    }
  }

  function startAutoRefresh() {
    stopAutoRefresh()
    refreshTimer = setInterval(() => {
      void refreshLogs()
    }, LOG_AUTO_REFRESH_INTERVAL_MS)
  }

  function toggleAutoRefresh() {
    autoRefresh.value = !autoRefresh.value
    if (autoRefresh.value) {
      startAutoRefresh()
    } else {
      stopAutoRefresh()
    }
  }

  onUnmounted(stopAutoRefresh)

  return {
    autoRefresh,
    startAutoRefresh,
    toggleAutoRefresh,
  }
}
