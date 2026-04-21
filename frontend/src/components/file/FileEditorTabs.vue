<template>
  <div class="editor-tabs">
    <div class="tabs-bar d-flex">
      <div
        v-for="tab in tabs"
        :key="tab.path"
        class="tab"
        :class="{ active: tab.path === activeTab, dragging: dragPath === tab.path }"
        draggable="true"
        @click="emit('activate', tab.path)"
        @auxclick="onAuxClick($event, tab.path)"
        @contextmenu.prevent="onContextMenu($event, tab.path)"
        @dragstart="onDragStart($event, tab.path)"
        @dragover.prevent
        @drop.prevent="onDrop($event, tab.path)"
        @dragend="onDragEnd"
      >
        <v-icon size="x-small" class="mr-1">mdi-file-outline</v-icon>
        <span class="tab-name">{{ basename(tab.path) }}</span>
        <v-icon
          size="x-small"
          class="tab-close ml-2"
          @click.stop="requestClose(tab.path)"
        >
          {{ isDirty(tab) ? 'mdi-circle-medium' : 'mdi-close' }}
        </v-icon>
      </div>
    </div>
    <v-progress-linear
      v-if="savingPath !== null"
      :model-value="(savingProgress ?? 0) * 100"
      color="primary"
      height="3"
    />
    <div class="editor-area" :class="{ saving: savingPath !== null }">
      <div v-if="!activeTab" class="welcome d-flex flex-column align-center justify-center">
        <v-icon size="64" color="grey-lighten-1">mdi-file-document-outline</v-icon>
        <div class="text-h6 mt-4 text-grey">从左侧选择文件打开</div>
      </div>
      <div v-else ref="editorContainer" class="editor-container"></div>
    </div>

    <v-menu
      v-model="tabMenuOpen"
      :target="[tabMenuX, tabMenuY]"
      close-on-content-click
    >
      <v-list density="compact" min-width="180">
        <v-list-item prepend-icon="mdi-close" @click="contextClose">
          <v-list-item-title>关闭</v-list-item-title>
        </v-list-item>
        <v-list-item prepend-icon="mdi-chevron-right" @click="contextCloseRight" :disabled="!hasTabsToRight">
          <v-list-item-title>关闭右侧</v-list-item-title>
        </v-list-item>
        <v-list-item prepend-icon="mdi-close-box-multiple" @click="contextCloseAll">
          <v-list-item-title>关闭全部</v-list-item-title>
        </v-list-item>
      </v-list>
    </v-menu>

    <ConfirmDialog ref="confirmDialog" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount, nextTick } from 'vue'
import * as monaco from 'monaco-editor'
import ConfirmDialog from '@/components/ConfirmDialog.vue'

export interface EditorTab {
  path: string
  content: string
  originalContent: string
}

const props = defineProps<{
  tabs: EditorTab[]
  activeTab: string | null
  savingPath?: string | null
  savingProgress?: number
}>()

const emit = defineEmits<{
  activate: [path: string]
  close: [path: string]
  'close-many': [paths: string[]]
  save: [path: string]
  'content-change': [payload: { path: string; content: string }]
  reorder: [payload: { src: string; dst: string }]
}>()

const editorContainer = ref<HTMLElement | null>(null)
const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()

let editor: monaco.editor.IStandaloneCodeEditor | null = null
const models = new Map<string, monaco.editor.ITextModel>()

// ----- Tab context menu -----
const tabMenuOpen = ref(false)
const tabMenuX = ref(0)
const tabMenuY = ref(0)
const tabMenuPath = ref<string | null>(null)

const hasTabsToRight = computed(() => {
  const target = tabMenuPath.value
  if (!target) return false
  const idx = props.tabs.findIndex((t) => t.path === target)
  return idx >= 0 && idx < props.tabs.length - 1
})

function onContextMenu(e: MouseEvent, path: string) {
  tabMenuPath.value = path
  tabMenuX.value = e.clientX
  tabMenuY.value = e.clientY
  tabMenuOpen.value = true
}

async function contextClose() {
  if (tabMenuPath.value) await requestClose(tabMenuPath.value)
}

async function contextCloseRight() {
  const target = tabMenuPath.value
  if (!target) return
  const idx = props.tabs.findIndex((t) => t.path === target)
  if (idx === -1) return
  const toClose = props.tabs.slice(idx + 1).map((t) => t.path)
  await closeMany(toClose)
}

async function contextCloseAll() {
  const toClose = props.tabs.map((t) => t.path)
  await closeMany(toClose)
}

async function closeMany(paths: string[]) {
  const dirtyPaths = paths.filter((p) => {
    const t = props.tabs.find((tb) => tb.path === p)
    return t && isDirty(t)
  })
  if (dirtyPaths.length > 0) {
    const confirmed = await confirmDialog.value!.open(
      '未保存的修改',
      `有 ${dirtyPaths.length} 个文件存在未保存的修改, 确定要关闭吗？（关闭后修改将丢失）`,
    )
    if (!confirmed) return
  }
  emit('close-many', paths)
}

// ----- Middle-click close -----
function onAuxClick(e: MouseEvent, path: string) {
  if (e.button === 1) {
    e.preventDefault()
    void requestClose(path)
  }
}

// ----- Drag-and-drop reorder -----
const dragPath = ref<string | null>(null)

function onDragStart(e: DragEvent, path: string) {
  dragPath.value = path
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', path)
  }
}

function onDrop(_e: DragEvent, dstPath: string) {
  const src = dragPath.value
  dragPath.value = null
  if (!src || src === dstPath) return
  emit('reorder', { src, dst: dstPath })
}

function onDragEnd() {
  dragPath.value = null
}

// ----- Helpers -----
function basename(path: string) {
  return path.split('/').pop() || path
}

function isDirty(tab: EditorTab) {
  return tab.content !== tab.originalContent
}

function getModel(tab: EditorTab): monaco.editor.ITextModel {
  let m = models.get(tab.path)
  if (!m) {
    const language = inferLanguage(tab.path)
    m = monaco.editor.createModel(tab.content, language)
    m.onDidChangeContent(() => {
      emit('content-change', { path: tab.path, content: m!.getValue() })
    })
    models.set(tab.path, m)
  } else if (m.getValue() !== tab.content) {
    m.setValue(tab.content)
  }
  return m
}

function inferLanguage(path: string): string {
  const ext = path.split('.').pop()?.toLowerCase()
  const map: Record<string, string> = {
    js: 'javascript',
    ts: 'typescript',
    html: 'html',
    css: 'css',
    json: 'json',
    md: 'markdown',
    py: 'python',
    sh: 'shell',
    txt: 'plaintext',
  }
  return ext ? map[ext] ?? 'plaintext' : 'plaintext'
}

async function mountEditor() {
  if (!props.activeTab) {
    // No active tab - editor DOM is unmounted via v-if; dispose editor instance
    if (editor) {
      editor.dispose()
      editor = null
    }
    return
  }
  await nextTick()
  if (!editorContainer.value) return

  const tab = props.tabs.find((t) => t.path === props.activeTab)
  if (!tab) return

  if (!editor) {
    editor = monaco.editor.create(editorContainer.value, {
      automaticLayout: true,
      theme: 'vs',
      tabSize: 2,
      fontSize: 14,
    })
  }
  editor.setModel(getModel(tab))
}

async function requestClose(path: string) {
  const tab = props.tabs.find((t) => t.path === path)
  if (tab && isDirty(tab)) {
    const confirmed = await confirmDialog.value!.open(
      '未保存的修改',
      `文件 "${basename(path)}" 有未保存的修改。确定关闭吗？（关闭后修改将丢失）`,
    )
    if (!confirmed) return
  }
  emit('close', path)
}

function onKeyDown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault()
    if (props.activeTab && props.savingPath === null) emit('save', props.activeTab)
  }
}

// ----- Read-only while saving active tab -----
watch(
  () => props.savingPath,
  (savingPath) => {
    if (!editor) return
    editor.updateOptions({ readOnly: savingPath !== null && savingPath === props.activeTab })
  },
)

watch(
  () => props.activeTab,
  () => {
    void mountEditor()
    if (editor) {
      editor.updateOptions({ readOnly: props.savingPath !== null && props.savingPath === props.activeTab })
    }
  },
)

watch(
  () => props.tabs.map((t) => t.path).join('|'),
  () => {
    // Drop models for closed tabs
    const activePaths = new Set(props.tabs.map((t) => t.path))
    for (const [p, m] of models.entries()) {
      if (!activePaths.has(p)) {
        m.dispose()
        models.delete(p)
      }
    }
  },
)

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  void mountEditor()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown)
  if (editor) {
    editor.dispose()
    editor = null
  }
  for (const m of models.values()) {
    m.dispose()
  }
  models.clear()
})

defineExpose({ requestClose })
</script>

<style scoped>
.editor-tabs {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.tabs-bar {
  background-color: #f0f0f0;
  border-bottom: 1px solid rgba(0, 0, 0, 0.12);
  overflow-x: auto;
}
.tab {
  display: flex;
  align-items: center;
  padding: 6px 10px;
  border-right: 1px solid rgba(0, 0, 0, 0.08);
  cursor: pointer;
  user-select: none;
  background-color: #ececec;
  white-space: nowrap;
}
.tab.active {
  background-color: #ffffff;
  border-bottom: 2px solid #1976d2;
}
.tab.dragging {
  opacity: 0.5;
}
.tab-name {
  font-size: 13px;
}
.tab-close {
  opacity: 0.6;
}
.tab-close:hover {
  opacity: 1;
}
.editor-area {
  flex: 1;
  min-height: 0;
  position: relative;
}
.editor-area.saving {
  pointer-events: none;
}
.editor-container {
  width: 100%;
  height: 100%;
}
.welcome {
  height: 100%;
}
</style>
