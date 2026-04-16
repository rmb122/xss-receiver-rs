<template>
  <v-container fluid>
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-folder-multiple</v-icon>
        文件管理
        <v-spacer />
        <v-btn color="primary" prepend-icon="mdi-refresh" @click="fetchFiles">刷新</v-btn>
        <v-btn
          color="primary"
          prepend-icon="mdi-folder-plus"
          @click="openCreateDirDialog"
          class="ml-2"
        >
          新建目录
        </v-btn>
      </v-card-title>

      <v-data-table
        :headers="fileHeaders"
        :items="tableItems"
        :group-by="[{ key: 'directory', order: 'asc' }]"
        :loading="loading"
        density="comfortable"
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
              <div class="d-flex align-center">
                <v-btn
                  :icon="isGroupOpen(item) ? '$expand' : '$next'"
                  color="medium-emphasis"
                  density="comfortable"
                  size="small"
                  variant="text"
                  @click="toggleGroup(item)"
                ></v-btn>
                <v-icon class="ml-2 mr-3" color="primary">mdi-folder-outline</v-icon>
                <span class="font-weight-bold">{{ item.value }}/</span>
                <v-spacer />
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="primary"
                  @click.stop="handleUploadClick(item.value)"
                >
                  <v-icon>mdi-upload</v-icon>
                  <v-tooltip activator="parent" location="top">上传文件</v-tooltip>
                </v-btn>
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="success"
                  @click.stop="handleCreateFileClick(item.value)"
                >
                  <v-icon>mdi-file-plus</v-icon>
                  <v-tooltip activator="parent" location="top">新建文件</v-tooltip>
                </v-btn>
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="warning"
                  @click.stop="openRenameDirDialog(item.value)"
                >
                  <v-icon>mdi-pencil</v-icon>
                  <v-tooltip activator="parent" location="top">重命名目录</v-tooltip>
                </v-btn>
                <v-btn
                  icon
                  size="small"
                  variant="text"
                  color="error"
                  @click.stop="handleDeleteDir(item.value)"
                >
                  <v-icon>mdi-delete</v-icon>
                  <v-tooltip activator="parent" location="top">删除目录</v-tooltip>
                </v-btn>
              </div>
            </td>
          </tr>
        </template>

        <template #item="{ item, columns }">
          <tr v-if="item.isEmpty">
            <td :colspan="columns.length" class="text-center text-grey py-3">
              <v-icon size="small" class="mr-1">mdi-folder-open-outline</v-icon>
              <span class="text-body-2">空目录</span>
            </td>
          </tr>
          <tr v-else>
            <td></td>
            <td>
              <v-icon size="small" class="mr-2">mdi-file-outline</v-icon>
              {{ item.name }}
            </td>
            <td>{{ formatFileSize(item.size) }}</td>
            <td class="text-center">{{ formatTime(item.modified_time) }}</td>
            <td class="text-center">
              <v-btn icon size="small" variant="text" color="primary" @click="handleEditFile(item)">
                <v-icon>mdi-pencil</v-icon>
                <v-tooltip activator="parent" location="top">编辑</v-tooltip>
              </v-btn>
              <v-btn
                icon
                size="small"
                variant="text"
                color="info"
                @click="handleDownloadFile(item)"
              >
                <v-icon>mdi-download</v-icon>
                <v-tooltip activator="parent" location="top">下载</v-tooltip>
              </v-btn>
              <v-btn
                icon
                size="small"
                variant="text"
                color="warning"
                @click="handleRenameFile(item)"
              >
                <v-icon>mdi-rename</v-icon>
                <v-tooltip activator="parent" location="top">重命名</v-tooltip>
              </v-btn>
              <v-btn icon size="small" variant="text" color="error" @click="handleDeleteFile(item)">
                <v-icon>mdi-delete</v-icon>
                <v-tooltip activator="parent" location="top">删除</v-tooltip>
              </v-btn>
            </td>
          </tr>
        </template>

        <template #no-data>
          <div class="text-center py-8">
            <v-icon size="64" color="grey">mdi-folder-open-outline</v-icon>
            <div class="text-h6 mt-4 text-grey">暂无文件数据</div>
            <div class="text-body-2 text-grey mt-2">请先创建一个目录</div>
          </div>
        </template>
      </v-data-table>
    </v-card>

    <!-- 创建目录对话框 -->
    <DirectoryFormDialog v-model="createDirDialog" mode="create" @submit="handleCreateDir" />

    <!-- 重命名目录对话框 -->
    <DirectoryFormDialog
      v-model="renameDirDialog"
      mode="rename"
      :current-name="currentDir"
      @submit="handleRenameDir"
    />

    <!-- 重命名文件对话框 -->
    <FileRenameDialog
      v-model="renameFileDialog"
      :current-directory="currentFile?.directory || ''"
      :current-name="currentFile?.name || ''"
      @submit="handleRenameFileSave"
    />

    <!-- 编辑文件对话框 -->
    <FileEditorDialog
      v-model="editDialog"
      :file-name="editingFile?.name || ''"
      :file-content="editingContent"
      :loading="saving"
      @save="handleSaveFile"
      @save-and-close="handleSaveFileAndClose"
    />

    <!-- 新建文件对话框 -->
    <FileCreateDialog v-model="createFileDialog" @submit="handleCreateFile" />

    <!-- 文件上传进度对话框 -->
    <FileUploadDialog
      v-model="uploadDialog"
      :file-name="uploadingFileName"
      :progress="uploadProgress * 100"
      @cancel="handleCancelUpload"
    />

    <!-- 隐藏的文件上传输入框 -->
    <input ref="fileInputRef" type="file" multiple hidden @change="handleFileSelect" />

    <ConfirmDialog ref="confirmDialog" />
  </v-container>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import {
  listAllFiles,
  createDirectory,
  deleteDirectory,
  renameDirectory,
  uploadFile,
  downloadFile,
  getFileContent,
  deleteFile,
  renameFile,
  chunkedUpload,
} from '@/api/file'
import { showErrorToast, showSuccessToast } from '@/utils/toast'
import { formatTime, formatFileSize } from '@/utils/format'
import { expandAllGroups } from '@/utils/table'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import DirectoryFormDialog from '@/components/file/DirectoryFormDialog.vue'
import FileRenameDialog from '@/components/file/FileRenameDialog.vue'
import FileEditorDialog from '@/components/file/FileEditorDialog.vue'
import FileCreateDialog from '@/components/file/FileCreateDialog.vue'
import FileUploadDialog from '@/components/file/FileUploadDialog.vue'
import type { FileList, FileTableItem } from '@/types/file'
import type { DataTableHeader } from 'vuetify'
import type { Group } from 'vuetify/lib/components/VDataTable/composables/group.mjs'

// --- 状态 ---
const loading = ref(false)
const saving = ref(false)
const fileMap = ref<FileList>({})
const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()
const fileInputRef = ref<HTMLInputElement | null>(null)
const currentUploadDir = ref<string | null>(null)
const groupHeaders = ref<
  Record<
    string,
    {
      item: Group<any>
      toggleGroup: (_: Group<any>) => void
      isGroupOpen: (_: Group<any>) => boolean
    }
  >
>({})

// 表格头部配置
const fileHeaders: DataTableHeader[] = [
  { title: '目录', key: 'data-table-group', width: '300px' },
  { title: '文件名', key: 'name' },
  { title: '大小', key: 'size', width: '130px' },
  { title: '修改时间', key: 'modified_time', width: '200px', align: 'center' },
  { title: '操作', key: 'actions', width: '200px', align: 'center', sortable: false },
]

// 转换文件映射为表格数据
const tableItems = computed<FileTableItem[]>(() => {
  return Object.entries(fileMap.value).flatMap(([directory, files]) => {
    // 如果目录为空，创建一个占位项以显示目录分组
    if (files.length === 0) {
      return [
        {
          name: '',
          size: 0,
          modified_time: 0,
          directory,
          path: directory,
          isEmpty: true, // 标记为空目录
        } as FileTableItem,
      ]
    }
    // 否则返回该目录下的所有文件
    return files.map((file) => ({
      ...file,
      directory,
      path: `${directory}/${file.name}`,
    }))
  })
})

// --- 数据加载 ---
async function fetchFiles() {
  loading.value = true
  try {
    const response = await listAllFiles()
    fileMap.value = response.data.payload || {}
    nextTick(() => {
      expandAllGroups(groupHeaders)
    })
  } finally {
    loading.value = false
  }
}

// --- 创建目录 ---
const createDirDialog = ref(false)

function openCreateDirDialog() {
  createDirDialog.value = true
}

async function handleCreateDir(dirName: string) {
  if (!dirName || dirName.includes('/')) return
  saving.value = true
  try {
    await createDirectory(dirName)
    showSuccessToast('目录创建成功')
    createDirDialog.value = false
    await fetchFiles()
  } finally {
    saving.value = false
  }
}

// --- 删除目录 ---
async function handleDeleteDir(dir: string) {
  const confirmed = await confirmDialog.value!.open(
    '确认删除目录',
    `确定要删除目录 "${dir}" 及其所有文件吗？此操作不可撤销。`,
  )
  if (!confirmed) return

  await deleteDirectory(dir)
  showSuccessToast('目录删除成功')
  await fetchFiles()
}

// --- 重命名目录 ---
const renameDirDialog = ref(false)
const currentDir = ref('')

function openRenameDirDialog(dir: string) {
  currentDir.value = dir
  renameDirDialog.value = true
}

async function handleRenameDir(newName: string) {
  if (!newName || newName.includes('/')) return
  const oldName = currentDir.value
  saving.value = true
  try {
    await renameDirectory(oldName, { new_name: newName })
    showSuccessToast('目录重命名成功')
    renameDirDialog.value = false
    await fetchFiles()
  } finally {
    saving.value = false
  }
}

// --- 上传文件 ---
const uploading = ref(false)
const uploadDialog = ref(false)
const uploadingFileName = ref('')
const uploadProgress = ref(0)

function handleUploadClick(directory: string) {
  currentUploadDir.value = directory
  fileInputRef.value?.click()
}

async function handleFileSelect(event: Event) {
  const input = event.target as HTMLInputElement
  if (!input.files || input.files.length === 0 || !currentUploadDir.value) return

  for (let i = 0; i < input.files.length; i++) {
    const file = input.files.item(i)
    if (file) {
      await doUpload(file)
    }
  }

  // 清空 input
  input.value = ''
  await fetchFiles()
}

async function doUpload(file: File) {
  if (!currentUploadDir.value) return
  uploading.value = true
  uploadDialog.value = true
  uploadingFileName.value = file.name
  uploadProgress.value = 0

  try {
    await chunkedUpload(currentUploadDir.value, file.name, file, (progress) => {
      uploadProgress.value = progress
    })
    showSuccessToast(`文件 "${file.name}" 上传成功`)
  } finally {
    uploading.value = false
    uploadDialog.value = false
    uploadProgress.value = 0
    uploadingFileName.value = ''
  }
}

// --- 取消上传 ---
function handleCancelUpload() {
  uploadDialog.value = false
  uploading.value = false
  uploadProgress.value = 0
  uploadingFileName.value = ''
  showErrorToast('上传已取消')
}

// --- 下载文件 ---
function handleDownloadFile(item: FileTableItem) {
  downloadFile(item.directory, item.name)
}

// --- 删除文件 ---
async function handleDeleteFile(item: FileTableItem) {
  const confirmed = await confirmDialog.value!.open(
    '确认删除文件',
    `确定要删除文件 "${item.name}" 吗？`,
  )
  if (!confirmed) return

  await deleteFile(item.directory, item.name)
  showSuccessToast('文件删除成功')
  await fetchFiles()
}

// --- 新建文件 ---
const createFileDialog = ref(false)
const currentCreateFileDir = ref<string | null>(null)

function handleCreateFileClick(directory: string) {
  currentCreateFileDir.value = directory
  createFileDialog.value = true
}

async function handleCreateFile(fileName: string) {
  if (!currentCreateFileDir.value) return

  // 使用空 Blob 创建文件
  const blob = new Blob([''], { type: 'text/plain' })
  await uploadFile(currentCreateFileDir.value, fileName, blob)
  showSuccessToast('文件创建成功')
  createFileDialog.value = false
  await fetchFiles()
}

// --- 重命名文件 ---
const renameFileDialog = ref(false)
const currentFile = ref<FileTableItem | null>(null)

function handleRenameFile(item: FileTableItem) {
  currentFile.value = item
  renameFileDialog.value = true
}

async function handleRenameFileSave(payload: { new_directory: string; new_name: string }) {
  if (!currentFile.value || !payload.new_name) return
  saving.value = true
  try {
    await renameFile(currentFile.value.directory, currentFile.value.name, {
      new_name: payload.new_name,
      ...(payload.new_directory ? { new_directory: payload.new_directory } : {}),
    })
    showSuccessToast('文件重命名成功')
    renameFileDialog.value = false
    await fetchFiles()
  } finally {
    saving.value = false
  }
}

// --- 编辑文件 ---
const editDialog = ref(false)
const editingFile = ref<FileTableItem | null>(null)
const editingContent = ref('')

async function handleEditFile(item: FileTableItem) {
  loading.value = true
  try {
    const response = await getFileContent(item.directory, item.name)
    editingContent.value = response.data as unknown as string
    editingFile.value = item
    editDialog.value = true
  } finally {
    loading.value = false
  }
}

async function handleSaveFile(content: string) {
  if (!editingFile.value) return
  saving.value = true
  try {
    const blob = new Blob([content], { type: 'text/plain' })
    await chunkedUpload(editingFile.value.directory, editingFile.value.name, blob)
    showSuccessToast('文件保存成功')
    await fetchFiles()
  } finally {
    saving.value = false
  }
}

async function handleSaveFileAndClose(content: string) {
  if (!editingFile.value) return
  saving.value = true
  try {
    const blob = new Blob([content], { type: 'text/plain' })
    await chunkedUpload(editingFile.value.directory, editingFile.value.name, blob)
    showSuccessToast('文件保存成功')
    editDialog.value = false
    await fetchFiles()
  } finally {
    saving.value = false
  }
}

// --- 初始化 ---
onMounted(() => {
  fetchFiles()
})
</script>

<style scoped>
.bg-grey-lighten-4 {
  background-color: rgb(245, 245, 245);
}
</style>
