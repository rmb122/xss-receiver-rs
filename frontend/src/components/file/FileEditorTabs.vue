<template>
  <div class="editor-tabs">
    <div class="tabs-bar d-flex">
      <div
        v-for="tab in tabs"
        :key="tab.path"
        class="tab"
        :class="{ active: tab.path === activeTab }"
        @click="emit('activate', tab.path)"
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
    <div class="editor-area">
      <div v-if="!activeTab" class="welcome d-flex flex-column align-center justify-center">
        <v-icon size="64" color="grey-lighten-1">mdi-file-document-outline</v-icon>
        <div class="text-h6 mt-4 text-grey">从左侧选择文件打开</div>
      </div>
      <div v-else ref="editorContainer" class="editor-container"></div>
    </div>
    <ConfirmDialog ref="confirmDialog" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount, nextTick } from 'vue'
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
}>()

const emit = defineEmits<{
  activate: [path: string]
  close: [path: string]
  save: [path: string]
  'content-change': [payload: { path: string; content: string }]
}>()

const editorContainer = ref<HTMLElement | null>(null)
const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()

let editor: monaco.editor.IStandaloneCodeEditor | null = null
const models = new Map<string, monaco.editor.ITextModel>()

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
  if (!props.activeTab) return
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
    if (props.activeTab) emit('save', props.activeTab)
  }
}

watch(
  () => props.activeTab,
  () => {
    void mountEditor()
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
.editor-container {
  width: 100%;
  height: 100%;
}
.welcome {
  height: 100%;
}
</style>
