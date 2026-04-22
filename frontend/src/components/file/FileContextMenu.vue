<template>
  <v-menu
    :model-value="modelValue"
    @update:model-value="emit('update:modelValue', $event)"
    :target="[x, y]"
    close-on-content-click
  >
    <v-list density="compact" min-width="180">
      <v-list-item
        v-for="item in items"
        :key="item.key"
        :prepend-icon="item.icon"
        @click="emit('select', item.key)"
      >
        <v-list-item-title>{{ item.label }}</v-list-item-title>
      </v-list-item>
    </v-list>
  </v-menu>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export type ContextMenuAction =
  | 'new-file'
  | 'new-dir'
  | 'rename'
  | 'upload'
  | 'delete'
  | 'refresh'
  | 'open'
  | 'download'
  | 'copy-path'

export type ContextMenuTarget = 'root' | 'directory' | 'file'

const props = defineProps<{
  modelValue: boolean
  target: ContextMenuTarget
  x: number
  y: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  select: [action: ContextMenuAction]
}>()

const items = computed<Array<{ key: ContextMenuAction; label: string; icon: string }>>(() => {
  switch (props.target) {
    case 'root':
      return [
        { key: 'new-file', label: '新建文件', icon: 'mdi-file-plus' },
        { key: 'new-dir', label: '新建子目录', icon: 'mdi-folder-plus' },
        { key: 'upload', label: '上传到此目录', icon: 'mdi-upload' },
        { key: 'refresh', label: '刷新', icon: 'mdi-refresh' },
      ]
    case 'directory':
      return [
        { key: 'new-file', label: '新建文件', icon: 'mdi-file-plus' },
        { key: 'new-dir', label: '新建子目录', icon: 'mdi-folder-plus' },
        { key: 'rename', label: '重命名', icon: 'mdi-rename' },
        { key: 'upload', label: '上传到此目录', icon: 'mdi-upload' },
        { key: 'copy-path', label: '复制路径', icon: 'mdi-content-copy' },
        { key: 'delete', label: '删除', icon: 'mdi-delete' },
        { key: 'refresh', label: '刷新', icon: 'mdi-refresh' },
      ]
    case 'file':
      return [
        { key: 'open', label: '打开编辑', icon: 'mdi-file-edit' },
        { key: 'download', label: '下载', icon: 'mdi-download' },
        { key: 'rename', label: '重命名', icon: 'mdi-rename' },
        { key: 'copy-path', label: '复制路径', icon: 'mdi-content-copy' },
        { key: 'delete', label: '删除', icon: 'mdi-delete' },
      ]
  }
})
</script>
