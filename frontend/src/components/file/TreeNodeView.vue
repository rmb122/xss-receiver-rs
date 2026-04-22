<template>
  <div>
    <div
      class="tree-node d-flex align-center"
      :style="{
        paddingLeft: `${indent * 12 + 2}px`,
        '--indent': indent,
      }"
      @click="handleClick"
      @contextmenu.prevent="handleContextMenu"
    >
      <v-icon v-if="node.kind === 'directory'" size="x-small" class="tree-chevron">
        {{ node.expanded ? 'mdi-chevron-down' : 'mdi-chevron-right' }}
      </v-icon>
      <span v-else class="tree-spacer" />
      <v-icon size="small" class="mr-1 tree-icon" :color="iconInfo.color">
        {{ iconInfo.icon }}
      </v-icon>
      <span class="tree-name text-body-2">{{ node.name }}</span>
    </div>
    <template v-if="node.kind === 'directory' && node.expanded && node.children">
      <TreeNodeView
        v-for="child in node.children"
        :key="child.path"
        :node="child"
        :indent="indent + 1"
        @toggle="(n) => emit('toggle', n)"
        @open-file="(p) => emit('open-file', p)"
        @context-menu="(p) => emit('context-menu', p)"
      />
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { TreeNode } from './FileExplorer.vue'
import { fileIcon, folderIcon } from '@/utils/fileIcon'

const props = defineProps<{
  node: TreeNode
  indent: number
}>()

const emit = defineEmits<{
  toggle: [node: TreeNode]
  'open-file': [path: string]
  'context-menu': [payload: { node: TreeNode; x: number; y: number }]
}>()

const iconInfo = computed(() => {
  if (props.node.kind === 'directory') {
    return folderIcon(props.node.expanded)
  }
  return fileIcon(props.node.name)
})

function handleClick() {
  if (props.node.kind === 'directory') {
    emit('toggle', props.node)
  } else {
    emit('open-file', props.node.path)
  }
}

function handleContextMenu(e: MouseEvent) {
  emit('context-menu', { node: props.node, x: e.clientX, y: e.clientY })
}
</script>

<style scoped>
.tree-node {
  cursor: pointer;
  user-select: none;
  height: 22px;
  position: relative;
}
.tree-node:hover {
  background-color: rgba(0, 0, 0, 0.06);
}
/*
 * Indent guide lines.
 * Each indent level is 12px wide; we paint a 1px vertical guide near the
 * right edge of each indent column so it sits just to the left of the
 * chevron / file-icon column, mimicking VS Code's tree guides.
 */
.tree-node::before {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  width: calc(var(--indent, 0) * 12px);
  pointer-events: none;
  background-image: repeating-linear-gradient(
    to right,
    transparent 0,
    transparent 9px,
    rgba(0, 0, 0, 0.18) 9px,
    rgba(0, 0, 0, 0.18) 10px,
    transparent 10px,
    transparent 12px
  );
  background-repeat: repeat-y;
}
.tree-chevron,
.tree-icon,
.tree-spacer {
  flex-shrink: 0;
}
.tree-spacer {
  display: inline-block;
  width: 16px;
}
.tree-name {
  flex: 1 1 auto;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
