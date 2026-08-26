<template>
  <v-container fluid>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-dns-outline</v-icon>
        DNS 日志
        <v-spacer />
        <v-btn color="primary" prepend-icon="mdi-refresh" @click="fetchLogs()"> 刷新 </v-btn>
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
        no-data-text="暂无 DNS 日志"
        loading-text="加载中..."
        @update:options="onOptionsUpdate"
        @click:row="handleRowClick"
      >
        <template #item.create_time="{ item }">
          {{ formatTime(item.create_time) }}
        </template>

        <template #expanded-row="{ columns, item }">
          <tr>
            <td :colspan="columns.length" class="pa-0">
              <div class="pa-4 bg-grey-lighten-5">
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
                      query_name: item.query_name,
                      query_type: item.query_type,
                      query_class: item.query_class,
                      create_time: item.create_time,
                    }"
                    max-height="260px"
                  />
                </div>

                <div v-if="item.extra_info !== null && item.extra_info !== undefined" class="mb-3">
                  <div class="text-subtitle-2 font-weight-bold mb-1">
                    <v-icon size="small" class="mr-1">mdi-plus-circle</v-icon>
                    额外信息
                  </div>
                  <JsonHighlight :data="item.extra_info" max-height="200px" />
                </div>

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
</template>

<script setup lang="ts">
import type { DataTableHeader } from 'vuetify'
import { getDnsLogs } from '@/api/dnsLog'
import type { DnsLog } from '@/types/dnsLog'
import JsonHighlight from '@/components/JsonHighlight.vue'
import { formatTime } from '@/utils/format'
import { useLogTable } from '@/composables/useLogTable'

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
  { title: '类型', key: 'query_type', width: '120px', align: 'center', sortable: false },
  { title: '域名', key: 'query_name', sortable: false },
]

const {
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
} = useLogTable<DnsLog>({
  fetchPage: getDnsLogs,
  notificationBody: '收到新的 DNS 查询',
  notificationTag: 'dns-log-notification',
})
</script>

<style scoped>
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
</style>
