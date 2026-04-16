<template>
  <v-dialog :model-value="modelValue" max-width="400" @update:model-value="updateModelValue">
    <v-card>
      <v-card-title>新建文件</v-card-title>
      <v-card-text>
        <v-text-field
          v-model="fileName"
          label="文件名"
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
        <v-btn color="success" variant="flat" :disabled="!isValid" @click="handleSubmit">
          创建
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'

// Props
interface Props {
  modelValue: boolean
}

const props = defineProps<Props>()

// Emits
interface Emits {
  (e: 'update:modelValue', value: boolean): void
  (e: 'submit', fileName: string): void
}

const emit = defineEmits<Emits>()

// 状态
const fileName = ref('')

// 验证规则
const fileNameRule = (value: string) => {
  if (!value) {
    return '文件名不能为空'
  }
  if (value.includes('/')) {
    return '文件名不能包含斜杠'
  }
  return true
}

// 计算属性：判断输入是否有效
const isValid = computed(() => {
  return fileName.value && !fileName.value.includes('/')
})

// 监听对话框打开，清空文件名
watch(
  () => props.modelValue,
  (newValue) => {
    if (newValue) {
      fileName.value = ''
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
    emit('submit', fileName.value)
  }
}
</script>
