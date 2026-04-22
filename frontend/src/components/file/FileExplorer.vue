<template>
  <div class="file-explorer">
    <div class="explorer-header d-flex align-center px-2 py-1">
      <span class="text-caption text-uppercase font-weight-bold">文件管理</span>
      <v-spacer />
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
import { listDir } from '@/api/file'
import type { Entry } from '@/types/file'
import TreeNodeView from './TreeNodeView.vue'

export interface TreeNode {
  path: string // '' for root, 'a/b/c' otherwise
  name: string
  kind: 'file' | 'directory'
  size: number
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
const expandedPaths = new Set<string>(loadExpandedPaths())

function loadExpandedPaths(): string[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return ['']
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.filter((s) => typeof s === 'string') : ['']
  } catch {
    return ['']
  }
}

function persistExpandedPaths() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...expandedPaths]))
  } catch {
    // ignore quota errors
  }
}

const rootNode = ref<TreeNode>({
  path: '',
  name: '/',
  kind: 'directory',
  size: 0,
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
        return existing
      }
      const childPath = node.path ? `${node.path}/${e.name}` : e.name
      const child: TreeNode = {
        path: childPath,
        name: e.name,
        kind: e.kind,
        size: e.size,
        loaded: false,
        expanded: false,
      }
      if (e.kind === 'directory' && expandedPaths.has(childPath)) {
        child.expanded = true
        try {
          await loadChildren(child)
        } catch {
          // stale path in storage — drop it and leave the child collapsed
          expandedPaths.delete(childPath)
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
  if (node.expanded) expandedPaths.add(node.path)
  else expandedPaths.delete(node.path)
  persistExpandedPaths()
}

async function refreshNode(node: TreeNode) {
  if (node.kind !== 'directory') return
  await loadChildren(node)
  node.expanded = true
  expandedPaths.add(node.path)
  persistExpandedPaths()
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
  persistExpandedPaths()
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
  flex: 1;
  overflow: auto;
  padding: 4px 0;
}
</style>
