<template>
  <v-dialog
    :model-value="modelValue"
    height="85vh"
    max-width="50vw"
    scrollable
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-file-edit</v-icon>
        编辑文件: {{ fileName }}
        <v-spacer />
        <v-btn icon size="small" variant="text" @click="$emit('update:modelValue', false)">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>
      <v-divider />
      <v-card-text class="pa-0">
        <MonacoEditor
          v-if="modelValue"
          :model-value="fileContent"
          :filename="fileName"
          height="100%"
          @update:model-value="handleContentChange"
        />
      </v-card-text>
      <v-divider />
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="$emit('update:modelValue', false)">取消</v-btn>
        <v-btn color="primary" variant="flat" :loading="loading" @click="handleSave"> 保存 </v-btn>
        <v-btn color="primary" variant="flat" :loading="loading" @click="handleSaveAndClose">
          保存并关闭
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import MonacoEditor from '@/components/MonacoEditor.vue'

interface Props {
  modelValue: boolean
  fileName: string
  fileContent: string
  loading?: boolean
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
  (e: 'save', content: string): void
  (e: 'save-and-close', content: string): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

// 内部编辑内容状态
const editContent = ref('')

// 监听 fileContent 变化，同步到内部状态
watch(
  () => props.fileContent,
  (newValue) => {
    editContent.value = newValue
  },
  { immediate: true },
)

// 处理内容变化
const handleContentChange = (newContent: string) => {
  editContent.value = newContent
}

// 处理保存
const handleSave = () => {
  emit('save', editContent.value)
}

// 处理保存并关闭
const handleSaveAndClose = () => {
  emit('save-and-close', editContent.value)
}
</script>
