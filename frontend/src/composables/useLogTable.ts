import { onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import type { LogQuery, PaginatedResponse } from '@/types/api'
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
  fetchPage: (params: LogQuery) => Promise<PaginatedResponse<T>>
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
  const filterInput = ref('')
  const appliedFilter = ref('')
  const filterError = ref('')
  const loadError = ref('')
  const applyingFilter = ref(false)
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone
  let lastMaxLog: { query: string; id: number } | undefined
  let lastTableOptions: ServerTableOptions | undefined
  let latestRequest = 0
  let disposed = false

  watch(filterInput, () => {
    filterError.value = ''
  })

  async function loadLogs(requestPage: number, filter: string, mode: 'refresh' | 'auto' | 'apply') {
    if (mode === 'auto' && loading.value) return
    const requestId = ++latestRequest
    const params = {
      page: requestPage,
      page_size: pageSize.value,
      filter: filter || undefined,
      timezone,
    }
    const query = JSON.stringify(params)
    loading.value = true
    applyingFilter.value = mode === 'apply'
    loadError.value = ''
    if (mode === 'apply') filterError.value = ''
    try {
      const payload = await options.fetchPage(params)
      if (requestId !== latestRequest) return

      if (mode === 'apply') {
        appliedFilter.value = filter
        expanded.value = []
        // The table emits its new page after this commit; that is not another query.
        lastTableOptions = { page: params.page, itemsPerPage: params.page_size }
        page.value = params.page
      }
      logs.value = payload.data
      total.value = payload.total

      const currentMaxId =
        payload.data.length > 0 ? Math.max(...payload.data.map((log) => log.id)) : -1
      if (
        mode === 'auto' &&
        payload.data.length > 0 &&
        lastMaxLog?.query === query &&
        currentMaxId > lastMaxLog.id
      ) {
        sendBrowserNotification({
          body: options.notificationBody,
          tag: options.notificationTag,
        })
      }

      lastMaxLog = { query, id: currentMaxId }
    } catch (error) {
      if (requestId !== latestRequest) return
      const message = error instanceof Error ? error.message : '日志加载失败'
      if (mode === 'apply') {
        if (filterInput.value.trim() === filter) filterError.value = message
      } else {
        loadError.value = message
      }
    } finally {
      if (requestId === latestRequest) {
        loading.value = false
        applyingFilter.value = false
      }
    }
  }

  function fetchLogs(isAutoRefresh = false) {
    return loadLogs(page.value, appliedFilter.value, isAutoRefresh ? 'auto' : 'refresh')
  }

  function applyFilter() {
    return loadLogs(1, filterInput.value.trim(), 'apply')
  }

  function clearFilter() {
    filterInput.value = ''
    return applyFilter()
  }

  function handleRowClick(_event: MouseEvent, item: { item: T }) {
    const logId = item.item.id.toString()
    expanded.value = expanded.value.length > 0 && expanded.value[0] === logId ? [] : [logId]
  }

  function onOptionsUpdate(tableOptions: ServerTableOptions) {
    page.value = tableOptions.page
    pageSize.value = tableOptions.itemsPerPage
    if (
      lastTableOptions?.page === tableOptions.page &&
      lastTableOptions.itemsPerPage === tableOptions.itemsPerPage
    )
      return
    lastTableOptions = { ...tableOptions }
    void fetchLogs()
  }

  const { autoRefresh, startAutoRefresh, toggleAutoRefresh } = useLogAutoRefresh(() =>
    fetchLogs(true),
  )

  onMounted(async () => {
    await requestBrowserNotificationPermission()
    if (!disposed && autoRefresh.value) {
      startAutoRefresh()
    }
  })

  onUnmounted(() => {
    disposed = true
    latestRequest++
  })

  return {
    logs,
    total,
    page,
    pageSize,
    loading,
    expanded,
    autoRefresh,
    filterInput,
    filterError,
    loadError,
    applyingFilter,
    applyFilter,
    clearFilter,
    fetchLogs,
    onOptionsUpdate,
    handleRowClick,
    toggleAutoRefresh,
  }
}
