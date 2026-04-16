<template>
  <v-dialog :model-value="modelValue" max-width="400" @update:model-value="updateModelValue">
    <v-card>
      <v-card-title>重命名文件</v-card-title>
      <v-card-text>
        <v-text-field
          v-model="fileName"
          label="文件路径"
          variant="outlined"
          density="compact"
          autofocus
          :rules="[fileNameRule]"
          @keydown.enter="handleSubmit"
        />
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="handleCancel">取消</v-btn>
        <v-btn color="primary" variant="flat" :disabled="!isValid" @click="handleSubmit">
          确定
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { showErrorToast } from '@/utils/toast'
import { ref, watch, computed } from 'vue'

// Props
interface Props {
  modelValue: boolean
  currentDirectory: string
  currentName: string
}

const props = defineProps<Props>()

// Emits
interface RenameSubmitPayload {
  new_directory: string
  new_name: string
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
  (e: 'submit', payload: RenameSubmitPayload): void
}

const emit = defineEmits<Emits>()

// 状态
const fileName = ref('')

// 验证规则
const fileNameRule = (value: string) => {
  if (!value) {
    return '文件名不能为空'
  }
  if (value.startsWith('/') || value.endsWith('/')) {
    return '文件名不能以斜杠开头或结尾'
  }
  if (value.includes('//')) {
    return '文件名不能包含连续的斜杠'
  }
  return true
}

// 计算属性：判断输入是否有效
const isValid = computed(() => {
  return (
    !!fileName.value &&
    !fileName.value.startsWith('/') &&
    !fileName.value.endsWith('/') &&
    !fileName.value.includes('//')
  )
})

// 监听对话框打开，自动填充当前文件名
watch(
  () => props.modelValue,
  (newValue) => {
    if (newValue) {
      fileName.value = props.currentDirectory + '/' + props.currentName
    }
  },
)

// 更新对话框显示状态
const updateModelValue = (value: boolean) => {
  emit('update:modelValue', value)
}

// 处理取消
const handleCancel = () => {
  emit('update:modelValue', false)
}

// 处理提交
const handleSubmit = () => {
  if (isValid.value) {
    const lastSlash = fileName.value.lastIndexOf('/')
    if (lastSlash === -1) {
      showErrorToast('无效的文件路径')
      return
    }

    emit('submit', {
      new_name: fileName.value.slice(lastSlash + 1),
      new_directory: fileName.value.slice(0, lastSlash),
    })
  }
}
</script>
