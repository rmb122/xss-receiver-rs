import { onMounted, ref, shallowRef } from 'vue'
import type { PaginatedRequest, PaginatedResponse } from '@/types/api'
import {
  requestBrowserNotificationPermission,
  sendBrowserNotification,
} from '@/utils/browserNotification'
import { useLogAutoRefresh } from '@/composables/useLogAutoRefresh'

interface LogRecord {
  id: number
}

interface ServerTableOptions {
  page: number
  itemsPerPage: number
}

interface UseLogTableOptions<T extends LogRecord> {
  fetchPage: (params: PaginatedRequest) => Promise<PaginatedResponse<T>>
  notificationBody: string
  notificationTag: string
}

export function useLogTable<T extends LogRecord>(options: UseLogTableOptions<T>) {
  const logs = shallowRef<T[]>([])
  const total = ref(0)
  const page = ref(1)
  const pageSize = ref(20)
  const loading = ref(false)
  const expanded = ref<readonly string[]>([])
  const lastMaxLog = ref<[number, number]>([-1, -1])

  async function fetchLogs(isAutoRefresh = false) {
    loading.value = true
    try {
      const payload = await options.fetchPage({ page: page.value, page_size: pageSize.value })
      logs.value = payload.data
      total.value = payload.total

      const currentMaxId =
        payload.data.length > 0 ? Math.max(...payload.data.map((log) => log.id)) : -1
      if (
        isAutoRefresh &&
        payload.data.length > 0 &&
        lastMaxLog.value[0] === page.value &&
        currentMaxId > lastMaxLog.value[1]
      ) {
        sendBrowserNotification({
          body: options.notificationBody,
          tag: options.notificationTag,
        })
      }

      lastMaxLog.value = [page.value, currentMaxId]
    } finally {
      loading.value = false
    }
  }

  function handleRowClick(_event: MouseEvent, item: { item: T }) {
    const logId = item.item.id.toString()
    expanded.value = expanded.value.length > 0 && expanded.value[0] === logId ? [] : [logId]
  }

  function onOptionsUpdate(tableOptions: ServerTableOptions) {
    page.value = tableOptions.page
    pageSize.value = tableOptions.itemsPerPage
    void fetchLogs()
  }

  const { autoRefresh, startAutoRefresh, toggleAutoRefresh } = useLogAutoRefresh(() =>
    fetchLogs(true),
  )

  onMounted(async () => {
    await requestBrowserNotificationPermission()
    if (autoRefresh.value) {
      startAutoRefresh()
    }
  })

  return {
    logs,
    total,
    page,
    pageSize,
    loading,
    expanded,
    autoRefresh,
    fetchLogs,
    onOptionsUpdate,
    handleRowClick,
    toggleAutoRefresh,
  }
}
