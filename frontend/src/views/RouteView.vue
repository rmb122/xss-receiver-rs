<template>
  <v-container fluid>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-routes</v-icon>
        HTTP 路由
        <v-spacer />
        <v-btn color="primary" prepend-icon="mdi-refresh" @click="fetchRoutes">刷新</v-btn>
        <v-btn
          color="primary"
          prepend-icon="mdi-format-list-group"
          @click="toggleGroupBy"
          class="ml-2"
        >
          {{ groupByEnabled ? '平铺显示' : '分组显示' }}
        </v-btn>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openCreateDialog" class="ml-2">
          新建路由
        </v-btn>
      </v-card-title>

      <v-data-table
        :headers="computedHeaders"
        :items="routes"
        :group-by="groupByEnabled ? [{ key: 'catalog', order: 'asc' }] : undefined"
        :loading="loading"
        density="comfortable"
        no-data-text="暂无路由数据"
        items-per-page="-1"
        hide-default-footer
      >
        <template v-slot:group-header="{ item, columns, toggleGroup, isGroupOpen }">
          <template
            :ref="
              (_el) => {
                groupHeaders[item.value] = { item, toggleGroup, isGroupOpen }
              }
            "
          />
          <tr>
            <td :colspan="columns.length" class="bg-grey-lighten-4">
              <div class="d-flex align-center py-2">
                <v-btn
                  :icon="isGroupOpen(item) ? '$expand' : '$next'"
                  color="medium-emphasis"
                  density="comfortable"
                  size="small"
                  variant="text"
                ></v-btn>
                <v-icon class="ml-2 mr-3" color="primary">mdi-folder-outline</v-icon>
                <span class="font-weight-bold">{{ item.value || '未分类' }}</span>
                <v-spacer />
                <v-btn
                  icon
                  density="comfortable"
                  size="small"
                  variant="text"
                  color="primary"
                  @click="openCreateDialogWithCatalog(item.value)"
                >
                  <v-icon>mdi-plus</v-icon>
                  <v-tooltip activator="parent" location="top">在此分类下新建路由</v-tooltip>
                </v-btn>
              </div>
            </td>
          </tr>
        </template>

        <template #item.timeout="{ item }">{{ item.timeout }} ms</template>

        <template #item.write_log="{ item }">
          <v-icon>{{ item.write_log ? 'mdi-check' : 'mdi-close' }}</v-icon>
        </template>

        <template #item.create_time="{ item }">
          {{ formatTime(item.create_time) }}
        </template>

        <template #item.actions="{ item }">
          <v-btn icon size="small" variant="text" color="primary" @click="openEditDialog(item)">
            <v-icon>mdi-pencil</v-icon>
          </v-btn>
          <v-btn icon size="small" variant="text" color="info" @click="openHandlerEditor(item)">
            <v-icon>mdi-file-edit</v-icon>
            <v-tooltip activator="parent" location="top">编辑 Handler 文件</v-tooltip>
          </v-btn>
          <v-btn icon size="small" variant="text" color="error" @click="handleDelete(item)">
            <v-icon>mdi-delete</v-icon>
          </v-btn>
        </template>
      </v-data-table>
    </v-card>

    <!-- 创建/编辑对话框 -->
    <v-dialog v-model="dialogVisible" max-width="600">
      <v-card>
        <v-card-title>{{ isEditing ? '编辑路由' : '新建路由' }}</v-card-title>
        <v-card-text>
          <v-select
            v-model="form.pattern_kind"
            :items="patternKindOptions"
            label="匹配方式"
            variant="outlined"
            density="compact"
            class="mb-2"
          />
          <v-text-field
            v-model="form.pattern"
            label="匹配路径"
            :placeholder="form.pattern_kind === 'PLAIN' ? '/path' : '^/.*$'"
            variant="outlined"
            density="compact"
            class="mb-2"
          />
          <v-select
            v-model="form.handler_kind"
            :items="handlerKindOptions"
            label="处理方式"
            variant="outlined"
            density="compact"
            class="mb-2"
          />
          <v-combobox
            v-model="form.handler"
            :items="handlerOptions"
            :loading="handlerLoading"
            label="处理文件"
            variant="outlined"
            density="compact"
            class="mb-2"
          />
          <v-text-field
            v-model.number="form.priority"
            label="优先级"
            type="number"
            :rules="[(v: number) => (v !== null && v !== undefined) || '优先级不能为空']"
            variant="outlined"
            density="compact"
            class="mb-2"
            required
            hint="匹配多个路由时, 将会使用优先级较大的"
          />
          <v-text-field
            v-model.number="form.timeout"
            label="超时 (ms)"
            type="number"
            variant="outlined"
            density="compact"
            class="mb-2"
          />
          <v-combobox
            v-model="form.catalog"
            :items="catalogOptions"
            label="分类"
            variant="outlined"
            density="compact"
            class="mb-2"
          />
          <v-switch
            v-model="form.write_log"
            label="记录日志"
            color="primary"
            density="compact"
            class="mb-2"
          />
          <v-text-field v-model="form.comment" label="备注" variant="outlined" density="compact" />
        </v-card-text>
        <v-card-actions>
          <v-spacer />
          <v-btn variant="text" @click="dialogVisible = false">取消</v-btn>
          <v-btn color="primary" variant="flat" :loading="saving" @click="handleSave">
            {{ isEditing ? '保存' : '创建' }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <ConfirmDialog ref="confirmDialog" />

    <!-- Handler 文件编辑器 -->
    <FileEditorDialog
      v-model="handlerEditorDialog"
      :file-name="editingHandlerFile.name"
      :file-content="editingHandlerFile.content"
      :loading="savingHandler"
      @save="handleSaveHandlerFile"
      @save-and-close="handleSaveHandlerFileAndClose"
    />
  </v-container>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import { getAllRoutes, createRoute, updateRoute, deleteRoute } from '@/api/route'
import { listAllFiles, getFileContent, chunkedUpload } from '@/api/file'
import { showSuccessToast, showErrorToast } from '@/utils/toast'
import { formatTime } from '@/utils/format'
import { expandAllGroups } from '@/utils/table'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import FileEditorDialog from '@/components/file/FileEditorDialog.vue'
import type { Route, HandlerKind, PatternKind } from '@/types/route'
import type { DataTableHeader } from 'vuetify'
import type { Group } from 'vuetify/lib/components/VDataTable/composables/group.mjs'

const ROUTE_GROUP_BY_KEY = 'XSS_RECEIVER_ROUTE_GROUP_BY'

const handlerKindOptions: HandlerKind[] = ['STATIC', 'SCRIPT', 'NONE']
const patternKindOptions: PatternKind[] = ['PLAIN', 'REGEX']

const routes = ref<Route[]>([])
const loading = ref(false)
const saving = ref(false)
const dialogVisible = ref(false)
const isEditing = ref(false)
const editingId = ref<number | null>(null)
const handlerOptions = ref<string[]>([])
const handlerLoading = ref(false)
const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()
const groupByEnabled = ref(true)
const groupHeaders = ref<
  {
    item: Group<any>
    toggleGroup: (_: Group<any>) => void
    isGroupOpen: (_: Group<any>) => boolean
  }[]
>([])

// Handler 文件编辑器相关状态
const handlerEditorDialog = ref(false)
const editingHandlerFile = ref({ name: '', content: '', path: '' })
const savingHandler = ref(false)

// 计算 headers，在分组模式下隐藏 catalog 列
const computedHeaders = computed(() => {
  const headers: DataTableHeader[] = [
    { title: 'ID', key: 'id', width: '60px', align: 'center' },
    { title: '匹配方式', key: 'pattern_kind', width: '180px', align: 'center' },
    { title: '匹配路径', key: 'pattern' },
    { title: '处理方式', key: 'handler_kind', width: '150px', align: 'center' },
    { title: '处理文件', key: 'handler' },
    { title: '优先级', key: 'priority', width: '100px', align: 'center' },
    { title: '超时时间', key: 'timeout', width: '130px' },
    { title: '记录日志', key: 'write_log', width: '130px', align: 'center' },
    { title: '备注', key: 'comment' },
    { title: '创建时间', key: 'create_time', width: '200px', align: 'center' },
    { title: '操作', key: 'actions', width: '160px', align: 'center', sortable: false },
  ]

  // 只有在非分组模式下才显示 catalog 列
  if (!groupByEnabled.value) {
    headers.splice(4, 0, { title: '分类', key: 'catalog', width: '130px' })
  } else {
    // 开启分组, 修改显示的名称
    headers.splice(0, 0, { title: '分类', key: 'data-table-group' })
  }

  return headers
})

// 从已有路由数据中提取不重复的分类值作为下拉选项
const catalogOptions = computed(() => {
  const catalogs = routes.value.map((r) => r.catalog).filter((c) => c !== '')
  return [...new Set(catalogs)].sort()
})

const form = ref({
  pattern_kind: 'PLAIN' as PatternKind,
  pattern: '',
  priority: 0,
  timeout: 5000,
  catalog: '',
  handler_kind: 'STATIC' as HandlerKind,
  handler: '',
  write_log: true,
  comment: '',
})

function resetForm() {
  form.value = {
    pattern_kind: 'PLAIN',
    pattern: '',
    priority: 0,
    timeout: 5000,
    catalog: '',
    handler_kind: 'STATIC',
    handler: '',
    write_log: true,
    comment: '',
  }
}

async function fetchRoutes() {
  loading.value = true
  try {
    const response = await getAllRoutes()
    routes.value = response.data.payload || []
    nextTick(() => {
      expandAllGroups(groupHeaders)
    })
  } finally {
    loading.value = false
  }
}

async function loadHandlerOptions() {
  handlerLoading.value = true
  try {
    const response = await listAllFiles()
    const files = response.data.payload
    if (files) {
      handlerOptions.value = Object.entries(files).flatMap(([dir, fileList]) =>
        fileList.map((f) => `${dir}/${f.name}`),
      )
    }
  } finally {
    handlerLoading.value = false
  }
}

function openCreateDialog() {
  isEditing.value = false
  editingId.value = null
  resetForm()
  dialogVisible.value = true
  loadHandlerOptions()
}

function openCreateDialogWithCatalog(catalog: string) {
  isEditing.value = false
  editingId.value = null
  resetForm()
  form.value.catalog = catalog
  dialogVisible.value = true
  loadHandlerOptions()
}

function openEditDialog(route: Route) {
  isEditing.value = true
  editingId.value = route.id
  form.value = {
    pattern_kind: route.pattern_kind,
    pattern: route.pattern,
    priority: route.priority,
    timeout: route.timeout,
    catalog: route.catalog,
    handler_kind: route.handler_kind,
    handler: route.handler,
    write_log: route.write_log,
    comment: route.comment,
  }
  dialogVisible.value = true
  loadHandlerOptions()
}

async function handleSave() {
  saving.value = true
  try {
    if (isEditing.value && editingId.value !== null) {
      await updateRoute({ route_id: editingId.value, ...form.value })
      showSuccessToast('路由更新成功')
    } else {
      await createRoute(form.value)
      showSuccessToast('路由创建成功')
    }
    dialogVisible.value = false
    fetchRoutes()
  } finally {
    saving.value = false
  }
}

async function handleDelete(route: Route) {
  const confirmed = await confirmDialog.value!.open(
    '确认删除',
    `确定要删除路由 "${route.pattern}" 吗？`,
  )
  if (!confirmed) return

  await deleteRoute({ route_id: route.id })
  showSuccessToast('路由删除成功')
  fetchRoutes()
}

// 打开 Handler 文件编辑器
async function openHandlerEditor(route: Route) {
  // handler 格式为 "directory/filename"，需要拆分
  const parts = route.handler.split('/')
  if (parts.length < 2) {
    showErrorToast('无效的文件路径')
    return
  }

  const directory = parts.slice(0, -1).join('/')
  const filename = parts[parts.length - 1]!

  const content = (await getFileContent(directory, filename)).data

  editingHandlerFile.value = {
    name: filename,
    content: content,
    path: route.handler,
  }
  handlerEditorDialog.value = true
}

// 保存 Handler 文件
async function handleSaveHandlerFile(content: string) {
  try {
    savingHandler.value = true

    // 拆分路径
    const parts = editingHandlerFile.value.path.split('/')
    if (parts.length < 2) {
      showErrorToast('无效的文件路径')
      return
    }

    const directory = parts.slice(0, -1).join('/')
    const filename = parts[parts.length - 1]!

    await chunkedUpload(directory, filename, new Blob([content], { type: 'text/plain' }))
    showSuccessToast('保存成功')
  } finally {
    savingHandler.value = false
  }
}

// 保存 Handler 文件并关闭编辑器
async function handleSaveHandlerFileAndClose(content: string) {
  try {
    savingHandler.value = true

    // 拆分路径
    const parts = editingHandlerFile.value.path.split('/')
    if (parts.length < 2) {
      showErrorToast('无效的文件路径')
      return
    }

    const directory = parts.slice(0, -1).join('/')
    const filename = parts[parts.length - 1]!

    await chunkedUpload(directory, filename, new Blob([content], { type: 'text/plain' }))
    showSuccessToast('保存成功')
    handlerEditorDialog.value = false
  } finally {
    savingHandler.value = false
  }
}

// 切换分组显示
function toggleGroupBy() {
  groupByEnabled.value = !groupByEnabled.value
  localStorage.setItem(ROUTE_GROUP_BY_KEY, JSON.stringify(groupByEnabled.value))
  nextTick(() => {
    expandAllGroups(groupHeaders)
  })
}

// 加载分组显示设置
function loadGroupBySetting() {
  const saved = localStorage.getItem(ROUTE_GROUP_BY_KEY)
  if (saved !== null) {
    groupByEnabled.value = JSON.parse(saved)
  }
}

onMounted(async () => {
  loadGroupBySetting()
  await fetchRoutes()
})
</script>

<style scoped>
.bg-grey-lighten-4 {
  background-color: rgb(245, 245, 245);
}
</style>
