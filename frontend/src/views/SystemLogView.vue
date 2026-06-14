<template>
  <v-container fluid>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-text-box-outline</v-icon>
        系统日志
      </v-card-title>
      <v-data-table-server
        v-model:items-per-page="pageSize"
        v-model:page="page"
        :headers="headers"
        :items="logs"
        :items-length="total"
        :loading="loading"
        item-value="id"
        density="comfortable"
        :items-per-page-options="[10, 20, 50, 100]"
        no-data-text="暂无系统日志"
        loading-text="加载中..."
        @update:options="onOptionsUpdate"
      >
        <template #item.log="{ item }">
          <span style="white-space: pre-wrap; word-break: break-all">{{ item.log }}</span>
        </template>

        <template #item.create_time="{ item }">
          {{ formatTime(item.create_time) }}
        </template>
      </v-data-table-server>
    </v-card>
  </v-container>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { getSystemLogs } from '@/api/systemLog'
import type { SystemLog } from '@/types/systemLog'
import type { DataTableHeader } from 'vuetify'
import { formatTime } from '@/utils/format'

const headers: DataTableHeader[] = [
  { title: 'ID', key: 'id', width: '80px', align: 'center', sortable: false },
  { title: '日志内容', key: 'log', sortable: false },
  { title: '创建时间', key: 'create_time', width: '200px', align: 'center', sortable: false },
]

const logs = ref<SystemLog[]>([])
const total = ref(0)
const page = ref(1)
const pageSize = ref(20)
const loading = ref(false)

async function fetchLogs() {
  loading.value = true
  try {
    const payload = await getSystemLogs({ page: page.value, page_size: pageSize.value })
    if (payload) {
      logs.value = payload.data
      total.value = payload.total
    }
  } finally {
    loading.value = false
  }
}

function onOptionsUpdate(options: any) {
  page.value = options.page
  pageSize.value = options.itemsPerPage
  fetchLogs()
}
</script>
