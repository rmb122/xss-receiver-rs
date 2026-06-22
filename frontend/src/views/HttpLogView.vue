<template>
  <v-container fluid>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-web</v-icon>
        HTTP 日志
        <v-spacer />
        <v-btn color="primary" prepend-icon="mdi-refresh" @click="handleManualRefresh">
          刷新
        </v-btn>
        <v-btn
          :color="autoRefresh ? 'primary' : undefined"
          prepend-icon="mdi-refresh-auto"
          @click="toggleAutoRefresh"
          class="ml-2"
        >
          自动刷新: {{ autoRefresh ? '开启' : '关闭' }}
        </v-btn>
      </v-card-title>
      <v-data-table-server
        v-model:items-per-page="pageSize"
        v-model:page="page"
        v-model:expanded="expanded"
        :headers="headers"
        :items="logs"
        :items-length="total"
        :loading="loading"
        :item-value="(item) => item.id.toString()"
        show-expand
        density="comfortable"
        :items-per-page-options="[10, 20, 50, 100]"
        no-data-text="暂无日志数据"
        loading-text="加载中..."
        @update:options="onOptionsUpdate"
        @click:row="handleRowClick"
      >
        <template #item.create_time="{ item }">
          {{ formatTime(item.create_time) }}
        </template>

        <template #item.user_agent="{ item }">
          {{ getUserAgent(item) }}
        </template>

        <template #item.summary="{ item }">
          <div class="summary-cell">{{ getDataSummary(item) }}</div>
        </template>

        <template #expanded-row="{ columns, item }">
          <tr>
            <td :colspan="columns.length" class="pa-0">
              <div class="pa-4 bg-grey-lighten-5">
                <!-- 基本信息 -->
                <div class="mb-3">
                  <div class="text-subtitle-2 font-weight-bold mb-1">
                    <v-icon size="small" class="mr-1">mdi-information</v-icon>
                    基本信息
                  </div>
                  <JsonHighlight
                    :data="{
                      id: item.id,
                      client_ip: item.client_ip,
                      client_port: item.client_port,
                      location: item.location,
                      method: item.method,
                      path: item.path,
                      parsed_body_type: item.parsed_body_type,
                      create_time: item.create_time,
                    }"
                    max-height="200px"
                  />
                </div>

                <!-- URL 参数 -->
                <div v-if="Object.keys(item.arg).length > 0" class="mb-3">
                  <div class="text-subtitle-2 font-weight-bold mb-1">
                    <v-icon size="small" class="mr-1">mdi-link-variant</v-icon>
                    URL 参数
                  </div>
                  <JsonHighlight :data="item.arg" max-height="200px" />
                </div>

                <!-- 请求头 -->
                <div v-if="Object.keys(item.header).length > 0" class="mb-3">
                  <div class="text-subtitle-2 font-weight-bold mb-1">
                    <v-icon size="small" class="mr-1">mdi-format-list-bulleted</v-icon>
                    请求头
                  </div>
                  <JsonHighlight :data="item.header" max-height="200px" />
                </div>

                <!-- 请求体 -->
                <div v-if="item.parsed_body_type !== 'NONE'" class="mb-3">
                  <div
                    class="text-subtitle-2 font-weight-bold mb-1 d-flex align-center justify-space-between"
                  >
                    <div>
                      <v-icon size="small" class="mr-1">mdi-code-braces</v-icon>
                      解析请求体 [{{ item.parsed_body_type }}]
                      <v-btn
                        v-if="item.parsed_body"
                        size="small"
                        icon="mdi-content-copy"
                        variant="text"
                        @click.stop="copyParsedBody(item)"
                      >
                      </v-btn>
                    </div>
                    <v-btn
                      size="small"
                      prepend-icon="mdi-file-eye-outline"
                      variant="text"
                      :loading="rawBodyLoadingId === item.id"
                      @click.stop="openRawBody(item)"
                    >
                      查看原始请求体
                    </v-btn>
                  </div>

                  <JsonHighlight
                    v-if="item.parsed_body_type === 'JSON' || item.parsed_body_type === 'FORM'"
                    :data="JSON.parse(item.parsed_body)"
                    max-height="200px"
                  />
                  <div
                    v-else-if="item.parsed_body_type === 'FAILED'"
                    class="text-body-2 text-warning"
                  >
                    请求体解析失败，请查看原始请求体
                  </div>
                </div>

                <!-- 文件 -->
                <div v-if="Object.keys(item.file).length > 0" class="mb-3">
                  <div class="text-subtitle-2 font-weight-bold mb-1">
                    <v-icon size="small" class="mr-1">mdi-file-multiple</v-icon>
                    文件 (file)
                  </div>
                  <div v-for="(files, fieldName) in item.file" :key="fieldName" class="ml-4 mb-1">
                    <strong>{{ fieldName }}:</strong>
                    <div v-for="([filename, hash], idx) in files" :key="idx" class="ml-4">
                      {{ filename }} —
                      <a :href="getDownloadLogFileUrl(hash)" target="_blank" class="text-primary">
                        <v-icon size="small">mdi-download</v-icon>
                        下载 ({{ hash }})
                      </a>
                    </div>
                  </div>
                </div>

                <!-- 额外信息 -->
                <div v-if="item.extra_info !== null && item.extra_info !== undefined" class="mb-3">
                  <div class="text-subtitle-2 font-weight-bold mb-1">
                    <v-icon size="small" class="mr-1">mdi-plus-circle</v-icon>
                    额外信息
                  </div>
                  <JsonHighlight :data="item.extra_info" max-height="200px" />
                </div>

                <!-- 错误日志 -->
                <div v-if="item.error_log" class="mb-3">
                  <div class="text-subtitle-2 font-weight-bold mb-1">
                    <v-icon size="small" color="error" class="mr-1">mdi-alert-circle</v-icon>
                    错误日志
                  </div>
                  <pre class="json-block error-block">{{ item.error_log }}</pre>
                </div>
              </div>
            </td>
          </tr>
        </template>
      </v-data-table-server>
    </v-card>
  </v-container>

  <v-dialog v-model="rawBodyDialog" max-width="1000px">
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-file-code-outline</v-icon>
        原始请求体
        <v-spacer />
        <v-btn icon="mdi-close" variant="text" @click="rawBodyDialog = false" />
      </v-card-title>
      <v-card-text>
        <MonacoEditor
          :model-value="rawBodyText"
          :encoding="rawBodyEncoding"
          :language="rawBodyLanguage"
          :filename="rawBodyFilename"
          height="60vh"
          read-only
          wrap-line
          @update:encoding="rawBodyEncoding = $event"
        />
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { UAParser } from 'ua-parser-js'
import { getHttpLogRawBody, getHttpLogs } from '@/api/httpLog'
import { getDownloadLogFileUrl } from '@/api/file'
import type { HttpLog } from '@/types/httpLog'
import JsonHighlight from '@/components/JsonHighlight.vue'
import MonacoEditor from '@/components/MonacoEditor.vue'
import { decodeBytes } from '@/utils/encoding'
import { showSuccessToast, showErrorToast } from '@/utils/toast'
import type { DataTableHeader } from 'vuetify'
import { formatTime } from '@/utils/format'

const headers: DataTableHeader[] = [
  { title: '', key: 'data-table-expand', width: '40px', align: 'center' },
  { title: 'ID', key: 'id', width: '70px', align: 'center', sortable: false },
  {
    title: '时间',
    key: 'create_time',
    width: '200px',
    nowrap: true,
    align: 'center',
    sortable: false,
  },
  {
    title: 'IP',
    key: 'client_ip',
    width: '180px',
    maxWidth: '180px',
    nowrap: true,
    align: 'center',
    sortable: false,
  },
  {
    title: '位置',
    key: 'location',
    width: '200px',
    maxWidth: '200px',
    nowrap: true,
    align: 'center',
    sortable: false,
  },
  {
    title: 'User-Agent',
    key: 'user_agent',
    width: '300px',
    maxWidth: '300px',
    nowrap: true,
    align: 'center',
    sortable: false,
  },
  {
    title: '方法',
    key: 'method',
    width: '100px',
    maxWidth: '100px',
    nowrap: true,
    align: 'center',
    sortable: false,
  },
  {
    title: '路径',
    key: 'path',
    width: '100px',
    maxWidth: '300px',
    nowrap: true,
    sortable: false,
  },
  { title: '数据摘要', key: 'summary', sortable: false },
]

const logs = ref<HttpLog[]>([])
const total = ref(0)
const page = ref(1)
const pageSize = ref(20)
const loading = ref(false)
const expanded = ref<readonly string[]>([])
const rawBodyDialog = ref(false)
const rawBodyContent = ref<Uint8Array<ArrayBuffer>>(new Uint8Array(0))
const rawBodyEncoding = ref('UTF-8')
const rawBodyText = computed(() => decodeBytes(rawBodyContent.value, rawBodyEncoding.value))
const rawBodyLanguage = ref('plaintext')
const rawBodyFilename = ref('raw-body.txt')
const rawBodyLoadingId = ref<number | null>(null)

// 刷新功能状态
const autoRefresh = ref<boolean>(true)
const refreshTimer = ref<ReturnType<typeof setInterval> | undefined>(undefined)

// 通知功能状态
const lastMaxLog = ref<[number, number]>([-1, -1]) // [页数, 上一次的最大日志 ID]

async function fetchLogs(isAutoRefresh = false) {
  loading.value = true
  try {
    const payload = await getHttpLogs({ page: page.value, page_size: pageSize.value })
    if (payload) {
      logs.value = payload.data
      total.value = payload.total

      const currentMaxId = Math.max(...payload.data.map((log) => log.id))

      // 检测是否有新请求（仅在自动刷新时触发通知）
      if (isAutoRefresh && payload.data.length > 0) {
        // 如果有新的日志 ID，发送通知
        // 页数相同, 且 id 更大
        if (lastMaxLog.value[0] == page.value && currentMaxId > lastMaxLog.value[1]) {
          sendNotification()
        }
      }

      // 更新最大日志 ID
      lastMaxLog.value = [page.value, currentMaxId]
    }
  } finally {
    loading.value = false
  }
}

function handleRowClick(_event: MouseEvent, item: { item: HttpLog }) {
  // vuetify expanded 强制用 string 作为 expanded key, 所以上面用 item-value 将 number 转换成了 string
  // 这里同理
  const logId = item.item.id.toString()

  // 如果点击的行已经展开，则收起
  if (expanded.value.length > 0 && expanded.value[0] === logId) {
    expanded.value = []
  } else {
    // 否则，收起之前的行，展开当前行
    expanded.value = [logId]
  }
}

function onOptionsUpdate(options: any) {
  page.value = options.page
  pageSize.value = options.itemsPerPage
  fetchLogs()
}

function getUserAgent(log: HttpLog): string {
  const ua = log.header?.['User-Agent']
  if (ua && ua.length > 0 && ua[0]) {
    const parser = new UAParser(ua[0])
    const result = parser.getResult()

    // 构建关键信息格式：操作系统/版本 浏览器引擎/版本
    const parts: string[] = []

    // 操作系统信息
    if (result.os.name) {
      if (result.os.version) {
        parts.push(`${result.os.name}/${result.os.version}`)
      } else {
        parts.push(result.os.name)
      }
    }

    // 浏览器信息
    if (result.browser.name) {
      if (result.browser.version) {
        parts.push(`${result.browser.name}/${result.browser.version}`)
      } else {
        parts.push(result.browser.name)
      }
    }

    return parts.length > 0 ? parts.join(' ') : ua[0]
  }
  return '未知浏览器'
}

function getDataSummary(log: HttpLog): string {
  const parts: string[] = []

  // GET keys
  const argKeys = Object.keys(log.arg || {})
  if (argKeys.length > 0) {
    parts.push(`GET: [${argKeys.join(', ')}]`)
  }

  // POST keys
  if (log.parsed_body) {
    if (log.parsed_body_type === 'FORM') {
      try {
        const bodyKeys = Object.keys(JSON.parse(log.parsed_body))
        if (bodyKeys.length > 0) {
          parts.push(`POST: [${bodyKeys.join(', ')}]`)
        }
      } catch {
        parts.push('POST: [FORM]')
      }
    } else if (log.parsed_body_type === 'JSON') {
      parts.push('POST: [JSON]')
    }
  } else if (log.parsed_body_type === 'FAILED') {
    parts.push('POST: [FAILED]')
  }

  // FILE keys
  const fileKeys = Object.keys(log.file || {})
  if (fileKeys.length > 0) {
    parts.push(`FILE: [${fileKeys.join(', ')}]`)
  }

  // COOKIE keys
  const cookieHeader = log.header?.['cookie']
  if (cookieHeader && cookieHeader.length > 0 && cookieHeader[0]) {
    try {
      const cookieNames = cookieHeader[0]
        .split(';')
        .map((c) => c.trim().split('=')[0])
        .filter(Boolean)
      if (cookieNames.length > 0) {
        parts.push(`COOKIE: [${cookieNames.join(', ')}]`)
      }
    } catch {
      // ignore
    }
  }

  return parts.join(' ')
}

const copyParsedBody = async (log: HttpLog) => {
  try {
    await navigator.clipboard.writeText(log.parsed_body)
    showSuccessToast('解析请求体已复制到剪贴板')
  } catch (error) {
    console.error('复制失败:', error)
    showErrorToast('复制失败', '无法访问剪贴板')
  }
}

function getHeaderValue(log: HttpLog, name: string): string {
  const target = name.toLowerCase()
  for (const [key, values] of Object.entries(log.header || {})) {
    if (key.toLowerCase() === target && values.length > 0) {
      return values[0] ?? ''
    }
  }
  return ''
}

function inferRawBodyLanguage(log: HttpLog): string {
  if (log.parsed_body_type === 'JSON') {
    return 'json'
  }

  const contentType = getHeaderValue(log, 'content-type').toLowerCase()
  if (contentType.includes('json')) {
    return 'json'
  }
  if (contentType.includes('html')) {
    return 'html'
  }
  if (contentType.includes('xml')) {
    return 'xml'
  }
  if (contentType.includes('css')) {
    return 'css'
  }
  if (contentType.includes('javascript') || contentType.includes('ecmascript')) {
    return 'javascript'
  }

  return 'plaintext'
}

async function openRawBody(log: HttpLog) {
  rawBodyLoadingId.value = log.id
  try {
    const body = await getHttpLogRawBody(log.id)
    rawBodyContent.value = new Uint8Array(body)
    rawBodyEncoding.value = 'UTF-8'
    rawBodyLanguage.value = inferRawBodyLanguage(log)
    rawBodyFilename.value = `http-log-${log.id}.body`
    rawBodyDialog.value = true
  } catch (error) {
    console.error('获取原始请求体失败:', error)
    showErrorToast('获取失败', '无法加载原始请求体')
  } finally {
    rawBodyLoadingId.value = null
  }
}

// 请求通知权限
async function requestNotificationPermission() {
  if (!('Notification' in window)) {
    console.log('此浏览器不支持桌面通知')
    return
  }

  if (Notification.permission === 'default') {
    await Notification.requestPermission()
  }
}

// 发送通知
function sendNotification() {
  if (!('Notification' in window)) {
    return
  }

  if (Notification.permission === 'granted') {
    new Notification('XSS Receiver', {
      body: '收到新 HTTP 请求',
      icon: '/favicon.ico',
      tag: 'http-log-notification',
    })
  }
}

// 手动刷新
function handleManualRefresh() {
  fetchLogs()
}

// 启动自动刷新
function startAutoRefresh() {
  // 清除已存在的定时器（防止重复）
  if (refreshTimer.value) {
    clearInterval(refreshTimer.value)
  }

  // 启动新的定时器（每 3 秒刷新一次）
  refreshTimer.value = setInterval(() => {
    fetchLogs(true) // 传递 true 表示这是自动刷新
  }, 3000)
}

// 停止自动刷新
function stopAutoRefresh() {
  if (refreshTimer.value) {
    clearInterval(refreshTimer.value)
    refreshTimer.value = undefined
  }
}

// 切换自动刷新
function toggleAutoRefresh() {
  autoRefresh.value = !autoRefresh.value

  if (autoRefresh.value) {
    startAutoRefresh()
  } else {
    stopAutoRefresh()
  }
}

// 组件挂载时，如果自动刷新是开启的，则启动定时器
onMounted(async () => {
  // 请求通知权限
  await requestNotificationPermission()

  if (autoRefresh.value) {
    startAutoRefresh()
  }
})

// 组件卸载时，清除定时器（防止内存泄漏）
onUnmounted(() => {
  stopAutoRefresh()
})
</script>

<style scoped>
:deep(.v-data-table tbody tr) {
  cursor: pointer;
}

:deep(.v-data-table tbody tr:hover) {
  background-color: rgba(0, 0, 0, 0.04);
}

.json-block {
  background-color: #f5f5f5;
  border: 1px solid #e0e0e0;
  border-radius: 4px;
  padding: 12px;
  font-size: 12px;
  line-height: 1.5;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

.error-block {
  background-color: #ffebee;
  border-color: #ef9a9a;
  color: #c62828;
}

.summary-cell {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  width: 0;
  min-width: 100%;
}
</style>
