<template>
  <div class="file-explorer">
    <div class="explorer-header d-flex align-center px-2 py-1">
      <span class="text-caption text-uppercase font-weight-bold">文件管理</span>
      <v-spacer />
      <v-btn icon size="x-small" variant="text" @click="collapseAll">
        <v-icon size="small">mdi-collapse-all</v-icon>
        <v-tooltip activator="parent" location="top">折叠全部目录</v-tooltip>
      </v-btn>
      <v-btn icon size="x-small" variant="text" @click="refreshNode(rootNode)">
        <v-icon size="small">mdi-refresh</v-icon>
        <v-tooltip activator="parent" location="top">刷新</v-tooltip>
      </v-btn>
    </div>
    <div class="explorer-body">
      <TreeNodeView
        :node="rootNode"
        :indent="0"
        @toggle="toggleNode"
        @open-file="(p) => emit('open-file', p)"
        @context-menu="onContextMenu"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useStorage } from '@vueuse/core'
import { listDir } from '@/api/file'
import type { Entry } from '@/types/file'
import TreeNodeView from './TreeNodeView.vue'

export interface TreeNode {
  path: string // '' for root, 'a/b/c' otherwise
  name: string
  kind: 'file' | 'directory'
  size: number
  modified_time: number
  loaded: boolean
  expanded: boolean
  children?: TreeNode[]
}

const emit = defineEmits<{
  'open-file': [path: string]
  'context-menu': [payload: { node: TreeNode; x: number; y: number }]
}>()

// ----- Persisted expanded state -----
// Stores the set of directory paths (relative) that are currently expanded,
// so the tree state survives a full page refresh.
const STORAGE_KEY = 'file-explorer-expanded'
const expandedPaths = useStorage<string[]>(STORAGE_KEY, [''], undefined, {
  listenToStorageChanges: false,
  onError: () => {},
})

function normalizeExpandedPaths(value: unknown): string[] {
  if (!Array.isArray(value)) return ['']
  const paths = value.filter((path): path is string => typeof path === 'string')
  if (!paths.includes('')) paths.unshift('')
  return [...new Set(paths)]
}

expandedPaths.value = normalizeExpandedPaths(expandedPaths.value)

function addExpandedPath(path: string) {
  if (!expandedPaths.value.includes(path)) {
    expandedPaths.value = [...expandedPaths.value, path]
  }
}

function removeExpandedPath(path: string) {
  expandedPaths.value = expandedPaths.value.filter((expandedPath) => expandedPath !== path)
}

const rootNode = ref<TreeNode>({
  path: '',
  name: '/',
  kind: 'directory',
  size: 0,
  modified_time: 0,
  loaded: false,
  expanded: true,
})

async function loadChildren(node: TreeNode) {
  const entries = await listDir(node.path)
  entries.sort((a, b) => {
    // directories first, then by name
    if (a.kind !== b.kind) return a.kind === 'directory' ? -1 : 1
    return a.name.localeCompare(b.name)
  })
  // Preserve existing children's expanded/loaded state when refreshing,
  // so saving a file doesn't collapse the tree.
  const oldChildren = new Map<string, TreeNode>()
  if (node.children) {
    for (const c of node.children) {
      oldChildren.set(c.name, c)
    }
  }

  // Build children and recursively load any that should be pre-expanded
  // (restored from localStorage). Done in parallel so all sibling subtrees
  // load concurrently, and inside loadChildren itself so the tree renders
  // in a single pass without a "collapsed then expand" flash.
  const nextChildren: TreeNode[] = await Promise.all(
    entries.map(async (e: Entry) => {
      const existing = oldChildren.get(e.name)
      if (existing && existing.kind === e.kind) {
        existing.size = e.size
        existing.modified_time = e.modified_time
        return existing
      }
      const childPath = node.path ? `${node.path}/${e.name}` : e.name
      const child: TreeNode = {
        path: childPath,
        name: e.name,
        kind: e.kind,
        size: e.size,
        modified_time: e.modified_time,
        loaded: false,
        expanded: false,
      }
      if (e.kind === 'directory' && expandedPaths.value.includes(childPath)) {
        child.expanded = true
        try {
          await loadChildren(child)
        } catch {
          // stale path in storage — drop it and leave the child collapsed
          removeExpandedPath(childPath)
          child.expanded = false
        }
      }
      return child
    }),
  )
  node.children = nextChildren
  node.loaded = true
}

async function toggleNode(node: TreeNode) {
  if (node.kind !== 'directory') return
  if (!node.loaded) {
    await loadChildren(node)
  }
  node.expanded = !node.expanded
  if (node.expanded) addExpandedPath(node.path)
  else removeExpandedPath(node.path)
}

async function refreshNode(node: TreeNode) {
  if (node.kind !== 'directory') return
  await loadChildren(node)
  node.expanded = true
  addExpandedPath(node.path)
}

// Recursively collapse every directory in the tree.
// Root stays expanded (it has no collapsed state — it's always the visible starting point).
function collapseAll() {
  const walk = (node: TreeNode) => {
    if (!node.children) return
    for (const c of node.children) {
      if (c.kind === 'directory') {
        c.expanded = false
        walk(c)
      }
    }
  }
  walk(rootNode.value)
  expandedPaths.value = ['']
}

function onContextMenu(payload: { node: TreeNode; x: number; y: number }) {
  emit('context-menu', payload)
}

function findNode(path: string): TreeNode | undefined {
  if (path === '') return rootNode.value
  const parts = path.split('/')
  let current: TreeNode = rootNode.value
  for (const p of parts) {
    if (!current.children) return undefined
    const next = current.children.find((c) => c.name === p)
    if (!next) return undefined
    current = next
  }
  return current
}

function findParent(path: string): TreeNode | undefined {
  if (!path.includes('/')) return rootNode.value
  const parentPath = path.substring(0, path.lastIndexOf('/'))
  return findNode(parentPath)
}

onMounted(async () => {
  await loadChildren(rootNode.value)
})

defineExpose({
  refreshNode,
  rootNode,
  findNode,
  findParent,
})
</script>

<style scoped>
.file-explorer {
  display: flex;
  flex-direction: column;
  height: 100%;
  border-right: 1px solid rgba(0, 0, 0, 0.12);
  background-color: #f8f8f8;
}
.explorer-header {
  border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  min-height: 32px;
}
.explorer-body {
  --explorer-scrollbar-thumb: rgba(100, 100, 100, 0.4);
  --explorer-scrollbar-thumb-hover: rgba(100, 100, 100, 0.7);
  --explorer-scrollbar-thumb-active: rgba(0, 0, 0, 0.6);

  flex: 1;
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
  padding: 4px 0;
  scrollbar-color: var(--explorer-scrollbar-thumb) transparent;
  scrollbar-width: thin;
}
.explorer-body::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
.explorer-body::-webkit-scrollbar-track,
.explorer-body::-webkit-scrollbar-corner {
  background-color: transparent;
}
.explorer-body::-webkit-scrollbar-thumb {
  background-color: var(--explorer-scrollbar-thumb);
}
.explorer-body::-webkit-scrollbar-thumb:hover {
  background-color: var(--explorer-scrollbar-thumb-hover);
}
.explorer-body::-webkit-scrollbar-thumb:active {
  background-color: var(--explorer-scrollbar-thumb-active);
}
.explorer-body::-webkit-scrollbar-button {
  display: none;
  width: 0;
  height: 0;
}
</style>
