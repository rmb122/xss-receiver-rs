<template>
  <div class="editor-tabs">
    <div class="tabs-bar d-flex">
      <div
        v-for="tab in tabs"
        :key="tab.path"
        class="tab"
        :class="{
          active: tab.path === activeTab,
          dragging: dragPath === tab.path,
          'drop-before': dragOverPath === tab.path && dragOverSide === 'before',
          'drop-after': dragOverPath === tab.path && dragOverSide === 'after',
        }"
        draggable="true"
        @click="emit('activate', tab.path)"
        @auxclick="onAuxClick($event, tab.path)"
        @contextmenu.prevent="onContextMenu($event, tab.path)"
        @dragstart="onDragStart($event, tab.path)"
        @dragover.prevent="onDragOver($event, tab.path)"
        @dragleave="onDragLeave(tab.path)"
        @drop.prevent="onDrop($event, tab.path)"
        @dragend="onDragEnd"
      >
        <v-icon size="x-small" class="mr-1" :color="tabIcon(tab.path).color">
          {{ tabIcon(tab.path).icon }}
        </v-icon>
        <span class="tab-name">{{ basename(tab.path) }}</span>
        <span v-if="tabHint(tab.path)" class="tab-hint ml-1">{{ tabHint(tab.path) }}</span>
        <v-icon size="x-small" class="tab-close ml-2" @click.stop="requestClose(tab.path)">
          {{ tab.dirty ? 'mdi-circle-medium' : 'mdi-close' }}
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
      <MonacoRawEditor
        v-else
        :model="activeModel"
        :encoding="activeTabEncoding"
        :read-only="savingPath !== null && savingPath === activeTab"
        height="100%"
        @content-change="onEditorContentChange"
        @update:encoding="onEditorEncodingChange"
      />
    </div>

    <v-menu v-model="tabMenuOpen" :target="[tabMenuX, tabMenuY]" close-on-content-click>
      <v-list density="compact" min-width="180">
        <v-list-item prepend-icon="mdi-close" @click="contextClose">
          <v-list-item-title>关闭</v-list-item-title>
        </v-list-item>
        <v-list-item
          prepend-icon="mdi-chevron-right"
          @click="contextCloseRight"
          :disabled="!hasTabsToRight"
        >
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
import { ref, shallowRef, markRaw, watch, computed, onMounted, onBeforeUnmount } from 'vue'
import { monaco } from '@/monaco'
import { fileIcon } from '@/utils/fileIcon'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import MonacoRawEditor from '@/components/MonacoRawEditor.vue'
import { decodeBytes } from '@/utils/encoding'

export interface EditorTab {
  path: string
  bytes: Uint8Array<ArrayBuffer>
  encoding: string
  dirty: boolean
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
  reorder: [payload: { src: string; dstIndex: number }]
}>()

const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()

const models = new Map<string, monaco.editor.ITextModel>()
const suppressDirtyPaths = new Set<string>()
// shallowRef + markRaw: never let Vue deeply proxy a monaco model, or its
// internals break and the editor spins / hangs the page.
const activeModel = shallowRef<monaco.editor.ITextModel | null>(null)

const activeTabEncoding = computed(() => {
  const tab = props.tabs.find((t) => t.path === props.activeTab)
  return tab?.encoding ?? 'UTF-8'
})

async function onEditorEncodingChange(enc: string) {
  if (!props.activeTab || enc === activeTabEncoding.value) return

  const tab = props.tabs.find((t) => t.path === props.activeTab)
  if (!tab) return

  if (tab.dirty) {
    const confirmed = await confirmDialog.value!.open(
      '切换编码',
      '切换编码会丢失当前未保存的修改，确定继续吗？',
    )
    if (!confirmed) return
  }

  tab.encoding = enc
  tab.dirty = false
  resetModel(tab)
}

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
    return t && t.dirty
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
const dragOverPath = ref<string | null>(null)
const dragOverSide = ref<'before' | 'after' | null>(null)

function onDragStart(e: DragEvent, path: string) {
  dragPath.value = path
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', path)
  }
}

function onDragOver(e: DragEvent, path: string) {
  if (!dragPath.value || dragPath.value === path) {
    dragOverPath.value = null
    dragOverSide.value = null
    return
  }
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
  const side: 'before' | 'after' = e.clientX < rect.left + rect.width / 2 ? 'before' : 'after'
  dragOverPath.value = path
  dragOverSide.value = side
}

function onDragLeave(path: string) {
  if (dragOverPath.value === path) {
    dragOverPath.value = null
    dragOverSide.value = null
  }
}

function onDrop(_e: DragEvent, dstPath: string) {
  const src = dragPath.value
  const side = dragOverSide.value
  dragPath.value = null
  dragOverPath.value = null
  dragOverSide.value = null
  if (!src || src === dstPath) return

  const tabs = props.tabs
  const srcIdx = tabs.findIndex((t) => t.path === src)
  const dstIdx = tabs.findIndex((t) => t.path === dstPath)
  if (srcIdx === -1 || dstIdx === -1) return

  const dstIndex = side === 'after' ? dstIdx + 1 : dstIdx
  if (dstIndex === srcIdx || dstIndex === srcIdx + 1) return
  emit('reorder', { src, dstIndex })
}

function onDragEnd() {
  dragPath.value = null
  dragOverPath.value = null
  dragOverSide.value = null
}

// ----- Helpers -----
function basename(path: string) {
  return path.split('/').pop() || path
}

function tabIcon(path: string) {
  return fileIcon(basename(path))
}

const tabHints = computed<Record<string, string>>(() => {
  const hints: Record<string, string> = {}
  const byName = new Map<string, string[]>()
  for (const t of props.tabs) {
    const name = basename(t.path)
    const list = byName.get(name)
    if (list) list.push(t.path)
    else byName.set(name, [t.path])
  }
  for (const [, paths] of byName) {
    if (paths.length < 2) continue
    const segs = paths.map((p) => p.split('/').slice(0, -1))
    for (let i = 0; i < paths.length; i++) {
      const mySegs = segs[i]!
      let depth = 1
      while (depth <= mySegs.length) {
        const mySuffix = mySegs.slice(-depth).join('/')
        const collides = segs.some((other, j) => {
          if (j === i) return false
          return other.slice(-depth).join('/') === mySuffix
        })
        if (!collides) break
        depth++
      }
      const suffix = mySegs.slice(-depth).join('/')
      hints[paths[i]!] = suffix || '/'
    }
  }
  return hints
})

function tabHint(path: string): string | null {
  return tabHints.value[path] ?? null
}

function getModel(tab: EditorTab): monaco.editor.ITextModel {
  let m = models.get(tab.path)
  if (!m) {
    const uri = monaco.Uri.file('/' + tab.path)
    const text = decodeBytes(tab.bytes, tab.encoding)
    m = markRaw(monaco.editor.createModel(text, undefined, uri))
    models.set(tab.path, m)
  }
  return m
}

function onEditorContentChange(model: monaco.editor.ITextModel) {
  const tab = props.tabs.find((t) => models.get(t.path) === model)
  if (!tab || suppressDirtyPaths.has(tab.path)) return
  if (!tab.dirty) {
    tab.dirty = true
  }
}

function syncActiveModel() {
  if (!props.activeTab) {
    activeModel.value = null
    return
  }
  const tab = props.tabs.find((t) => t.path === props.activeTab)
  activeModel.value = tab ? getModel(tab) : null
}

async function requestClose(path: string) {
  const tab = props.tabs.find((t) => t.path === path)
  if (tab && tab.dirty) {
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

watch(
  () => props.activeTab,
  () => {
    syncActiveModel()
  },
)

watch(
  () => props.tabs.map((t) => t.path).join('|'),
  () => {
    syncActiveModel()
    const activePaths = new Set(props.tabs.map((t) => t.path))
    for (const [p, m] of models.entries()) {
      if (!activePaths.has(p)) {
        if (activeModel.value === m) activeModel.value = null
        m.dispose()
        models.delete(p)
      }
    }
  },
)

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  syncActiveModel()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown)
  for (const m of models.values()) {
    m.dispose()
  }
  models.clear()
})

function getContent(path: string): string | null {
  const m = models.get(path)
  return m ? m.getValue() : null
}

function resetModel(tab: EditorTab) {
  const m = models.get(tab.path)
  if (m) {
    const text = decodeBytes(tab.bytes, tab.encoding)
    suppressDirtyPaths.add(tab.path)
    try {
      m.setValue(text)
    } finally {
      suppressDirtyPaths.delete(tab.path)
    }
  }
}

defineExpose({ requestClose, getContent })
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
  position: relative;
}
.tab.active {
  background-color: #ffffff;
  border-bottom: 2px solid #1976d2;
}
.tab.dragging {
  opacity: 0.5;
}
.tab.drop-before::before,
.tab.drop-after::after {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  background-color: #1976d2;
  pointer-events: none;
  z-index: 1;
}
.tab.drop-before::before {
  left: -1px;
}
.tab.drop-after::after {
  right: -1px;
}
.tab-name {
  font-size: 13px;
}
.tab-hint {
  font-size: 12px;
  color: rgba(0, 0, 0, 0.5);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 160px;
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
.welcome {
  height: 100%;
}
</style>
