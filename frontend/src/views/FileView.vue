<template>
  <v-container fluid class="file-view-container pa-0">
    <div class="file-view-layout">
      <div class="explorer-pane">
        <FileExplorer ref="explorer" @open-file="openFile" @context-menu="onContextMenu" />
      </div>
      <div class="editor-pane">
        <FileEditorTabs
          ref="editorTabs"
          :tabs="tabs"
          :active-tab="activeTab"
          :saving-path="savingPath"
          :saving-progress="savingProgress"
          @activate="setActive"
          @close="closeTab"
          @close-many="closeManyTabs"
          @save="saveTab"
          @content-change="onContentChange"
          @reorder="reorderTab"
        />
      </div>
    </div>

    <FileContextMenu
      v-model="menuOpen"
      :target="menuTarget"
      :x="menuX"
      :y="menuY"
      @select="onMenuSelect"
    />

    <DirectoryFormDialog
      v-model="formDialog"
      :title="formTitle"
      :label="formLabel"
      :placeholder="formPlaceholder"
      :initial-value="formInitial"
      @submit="onFormSubmit"
    />

    <FileUploadDialog
      v-model="uploadDialog"
      :file-name="uploadFileName"
      :progress="uploadProgress * 100"
      @cancel="cancelUpload"
    />

    <ConfirmDialog ref="confirmDialog" />

    <input ref="fileInputRef" type="file" multiple hidden @change="onFileSelect" />
  </v-container>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import FileExplorer, { type TreeNode } from '@/components/file/FileExplorer.vue'
import FileContextMenu, { type ContextMenuAction, type ContextMenuTarget } from '@/components/file/FileContextMenu.vue'
import FileEditorTabs, { type EditorTab } from '@/components/file/FileEditorTabs.vue'
import DirectoryFormDialog from '@/components/file/DirectoryFormDialog.vue'
import FileUploadDialog from '@/components/file/FileUploadDialog.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import {
  mkdir,
  remove as apiRemove,
  rename as apiRename,
  getFileContent,
  chunkedUpload,
  downloadFile,
} from '@/api/file'
import { showErrorToast, showSuccessToast } from '@/utils/toast'

const explorer = ref<InstanceType<typeof FileExplorer>>()
const editorTabs = ref<InstanceType<typeof FileEditorTabs>>()
const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()
const fileInputRef = ref<HTMLInputElement | null>(null)

// ----- Tabs -----
const tabs = ref<EditorTab[]>([])
const activeTab = ref<string | null>(null)

function setActive(path: string) {
  activeTab.value = path
}

function onContentChange(payload: { path: string; content: string }) {
  const t = tabs.value.find((t) => t.path === payload.path)
  if (t) t.content = payload.content
}

const MAX_EDIT_FILE_SIZE = 3 * 1024 * 1024 // 3 MB

async function openFile(path: string) {
  const existing = tabs.value.find((t) => t.path === path)
  if (existing) {
    activeTab.value = path
    return
  }
  // Check file size before fetching content
  const node = explorer.value?.findNode(path)
  if (node && node.size > MAX_EDIT_FILE_SIZE) {
    showErrorToast(`文件过大 (${(node.size / 1024 / 1024).toFixed(2)} MB), 无法在线编辑, 请下载后查看`)
    return
  }
  const content = await getFileContent(path)
  tabs.value.push({ path, content, originalContent: content })
  activeTab.value = path
}

function closeTab(path: string) {
  const idx = tabs.value.findIndex((t) => t.path === path)
  if (idx === -1) return
  tabs.value.splice(idx, 1)
  if (activeTab.value === path) {
    activeTab.value = tabs.value.length > 0 ? tabs.value[Math.max(0, idx - 1)]!.path : null
  }
}

function forceCloseTab(path: string) {
  const idx = tabs.value.findIndex((t) => t.path === path)
  if (idx === -1) return
  tabs.value.splice(idx, 1)
  if (activeTab.value === path) {
    activeTab.value = tabs.value.length > 0 ? tabs.value[Math.max(0, idx - 1)]!.path : null
  }
}

function closeManyTabs(paths: string[]) {
  const pathSet = new Set(paths)
  const wasActiveClosed = activeTab.value !== null && pathSet.has(activeTab.value)
  const oldActiveIdx = activeTab.value !== null ? tabs.value.findIndex((t) => t.path === activeTab.value) : -1
  tabs.value = tabs.value.filter((t) => !pathSet.has(t.path))
  if (wasActiveClosed) {
    if (tabs.value.length === 0) {
      activeTab.value = null
    } else {
      const newIdx = Math.min(Math.max(0, oldActiveIdx - paths.length + 1), tabs.value.length - 1)
      activeTab.value = tabs.value[Math.max(0, newIdx)]!.path
    }
  }
}

function reorderTab(payload: { src: string; dst: string }) {
  const srcIdx = tabs.value.findIndex((t) => t.path === payload.src)
  const dstIdx = tabs.value.findIndex((t) => t.path === payload.dst)
  if (srcIdx === -1 || dstIdx === -1) return
  const [moved] = tabs.value.splice(srcIdx, 1)
  tabs.value.splice(dstIdx, 0, moved!)
}

// ----- Save (linear progress bar, editor disabled while saving) -----
const savingPath = ref<string | null>(null)
const savingProgress = ref(0)

async function saveTab(path: string) {
  const tab = tabs.value.find((t) => t.path === path)
  if (!tab) return
  if (savingPath.value !== null) return
  savingPath.value = path
  savingProgress.value = 0
  try {
    const blob = new Blob([tab.content], { type: 'text/plain' })
    await chunkedUpload(path, blob, (p) => {
      savingProgress.value = p
    })
    tab.originalContent = tab.content
    // refresh parent directory in tree (size/modified_time may have changed)
    const parent = explorer.value?.findParent(path)
    if (parent) await explorer.value?.refreshNode(parent)
  } finally {
    savingPath.value = null
    savingProgress.value = 0
  }
}

// ----- Context menu -----
const menuOpen = ref(false)
const menuTarget = ref<ContextMenuTarget>('root')
const menuX = ref(0)
const menuY = ref(0)
const menuNode = ref<TreeNode | null>(null)

function onContextMenu(payload: { node: TreeNode; x: number; y: number }) {
  menuNode.value = payload.node
  menuX.value = payload.x
  menuY.value = payload.y
  if (payload.node.path === '') menuTarget.value = 'root'
  else if (payload.node.kind === 'directory') menuTarget.value = 'directory'
  else menuTarget.value = 'file'
  menuOpen.value = true
}

async function onMenuSelect(action: ContextMenuAction) {
  menuOpen.value = false
  const node = menuNode.value
  if (!node) return

  switch (action) {
    case 'new-file':
      openForm('new-file', node)
      break
    case 'new-dir':
      openForm('new-dir', node)
      break
    case 'rename':
      openForm('rename', node)
      break
    case 'delete':
      await handleDelete(node)
      break
    case 'refresh':
      await explorer.value?.refreshNode(node)
      break
    case 'upload':
      pendingUploadDir.value = node.path
      fileInputRef.value?.click()
      break
    case 'open':
      if (node.kind === 'file') await openFile(node.path)
      break
    case 'download':
      if (node.kind === 'file') downloadFile(node.path)
      break
  }
}

async function handleDelete(node: TreeNode) {
  const confirmed = await confirmDialog.value!.open(
    '确认删除',
    `确定要删除 "${node.path}" 吗？此操作不可撤销。`,
  )
  if (!confirmed) return
  await apiRemove(node.path)
  showSuccessToast('删除成功')
  // close any open tabs under this path
  tabs.value
    .filter((t) => t.path === node.path || t.path.startsWith(node.path + '/'))
    .forEach((t) => forceCloseTab(t.path))
  const parent = explorer.value?.findParent(node.path)
  if (parent) await explorer.value?.refreshNode(parent)
}

// ----- Form dialog (new file / new dir / rename) -----
type FormMode = 'new-file' | 'new-dir' | 'rename'
const formDialog = ref(false)
const formMode = ref<FormMode>('new-file')
const formTarget = ref<TreeNode | null>(null)
const formTitle = ref('')
const formLabel = ref('')
const formPlaceholder = ref('')
const formInitial = ref('')

function openForm(mode: FormMode, node: TreeNode) {
  formMode.value = mode
  formTarget.value = node
  switch (mode) {
    case 'new-file':
      formTitle.value = '新建文件'
      formLabel.value = '文件名'
      formPlaceholder.value = 'example.js'
      formInitial.value = ''
      break
    case 'new-dir':
      formTitle.value = '新建目录'
      formLabel.value = '目录名'
      formPlaceholder.value = 'new-folder'
      formInitial.value = ''
      break
    case 'rename':
      formTitle.value = '重命名'
      formLabel.value = '新完整路径'
      formPlaceholder.value = node.path
      formInitial.value = node.path
      break
  }
  formDialog.value = true
}

async function onFormSubmit(value: string) {
  const target = formTarget.value
  if (!target) return

  try {
    switch (formMode.value) {
      case 'new-file': {
        const newPath = target.path ? `${target.path}/${value}` : value
        const blob = new Blob([''], { type: 'text/plain' })
        await chunkedUpload(newPath, blob)
        showSuccessToast('文件创建成功')
        await explorer.value?.refreshNode(target)
        break
      }
      case 'new-dir': {
        const newPath = target.path ? `${target.path}/${value}` : value
        await mkdir(newPath)
        showSuccessToast('目录创建成功')
        await explorer.value?.refreshNode(target)
        break
      }
      case 'rename': {
        const oldPath = target.path
        await apiRename(oldPath, value)
        showSuccessToast('重命名成功')
        // force-close any tabs under old path
        tabs.value
          .filter((t) => t.path === oldPath || t.path.startsWith(oldPath + '/'))
          .forEach((t) => forceCloseTab(t.path))
        const oldParent = explorer.value?.findParent(oldPath)
        const newParent = explorer.value?.findParent(value)
        if (oldParent) await explorer.value?.refreshNode(oldParent)
        if (newParent && newParent !== oldParent) await explorer.value?.refreshNode(newParent)
        break
      }
    }
    formDialog.value = false
  } catch (e) {
    showErrorToast((e as Error).message || '操作失败')
  }
}

// ----- Upload -----
const uploadDialog = ref(false)
const uploadFileName = ref('')
const uploadProgress = ref(0)
const pendingUploadDir = ref<string | null>(null)

async function onFileSelect(e: Event) {
  const input = e.target as HTMLInputElement
  if (!input.files || input.files.length === 0 || pendingUploadDir.value === null) return
  const dir = pendingUploadDir.value

  for (let i = 0; i < input.files.length; i++) {
    const file = input.files.item(i)
    if (!file) continue
    await doUpload(dir, file)
  }
  input.value = ''
  pendingUploadDir.value = null

  const node = explorer.value?.findNode(dir)
  if (node) await explorer.value?.refreshNode(node)
}

async function doUpload(dir: string, file: File) {
  const path = dir ? `${dir}/${file.name}` : file.name
  uploadFileName.value = file.name
  uploadProgress.value = 0
  uploadDialog.value = true
  try {
    await chunkedUpload(path, file, (p) => {
      uploadProgress.value = p
    })
    showSuccessToast(`"${file.name}" 上传成功`)
  } finally {
    uploadDialog.value = false
  }
}

function cancelUpload() {
  uploadDialog.value = false
  uploadProgress.value = 0
  showErrorToast('已取消')
}
</script>

<style scoped>
.file-view-container {
  height: calc(100vh - 64px);
  max-width: none !important;
}
.file-view-layout {
  display: flex;
  height: 100%;
}
.explorer-pane {
  width: 280px;
  min-width: 200px;
  flex-shrink: 0;
}
.editor-pane {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
</style>
