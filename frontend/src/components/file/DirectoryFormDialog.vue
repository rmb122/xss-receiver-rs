<template>
  <v-dialog :model-value="modelValue" max-width="400" @update:model-value="handleUpdateModelValue">
    <v-card>
      <v-card-title>{{ dialogTitle }}</v-card-title>
      <v-card-text>
        <v-text-field
          v-model="dirName"
          :label="inputLabel"
          variant="outlined"
          density="compact"
          autofocus
          :rules="[nameRule]"
          @keydown.enter="handleSubmit"
        />
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="handleCancel">取消</v-btn>
        <v-btn
          color="primary"
          variant="flat"
          :disabled="!dirName || dirName.includes('/')"
          @click="handleSubmit"
        >
          {{ submitButtonText }}
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'

interface Props {
  modelValue: boolean
  mode: 'create' | 'rename'
  currentName?: string
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
  (e: 'submit', dirName: string): void
}

const props = withDefaults(defineProps<Props>(), {
  currentName: '',
})

const emit = defineEmits<Emits>()

const dirName = ref('')

// 监听 modelValue 变化，在对话框打开时初始化输入值
watch(
  () => props.modelValue,
  (newValue) => {
    if (newValue) {
      dirName.value = props.mode === 'rename' ? props.currentName || '' : ''
    }
  },
)

// 监听 currentName 变化
watch(
  () => props.currentName,
  (newValue) => {
    if (props.modelValue && props.mode === 'rename') {
      dirName.value = newValue || ''
    }
  },
)

// 计算属性：对话框标题
const dialogTitle = computed(() => {
  return props.mode === 'create' ? '新建目录' : '重命名目录'
})

// 计算属性：输入框标签
const inputLabel = computed(() => {
  return props.mode === 'create' ? '目录名称' : '新目录名称'
})

// 计算属性：提交按钮文本
const submitButtonText = computed(() => {
  return props.mode === 'create' ? '创建' : '确定'
})

// 验证规则
const nameRule = (value: string) => {
  if (!value) {
    return '名称不能为空'
  }
  if (value.includes('/')) {
    return '名称不能包含斜杠'
  }
  return true
}

// 处理对话框显示状态变化
const handleUpdateModelValue = (value: boolean) => {
  emit('update:modelValue', value)
}

// 处理取消
const handleCancel = () => {
  emit('update:modelValue', false)
}

// 处理提交
const handleSubmit = () => {
  if (dirName.value && !dirName.value.includes('/')) {
    emit('submit', dirName.value)
  }
}
</script>
