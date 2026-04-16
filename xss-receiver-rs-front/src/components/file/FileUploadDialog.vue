<template>
  <v-dialog
    :model-value="modelValue"
    @update:model-value="$emit('update:modelValue', $event)"
    max-width="400"
    persistent
  >
    <v-card :loading="progress < 100 && progress > 0" :progress="progress">
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-upload</v-icon>
        上传文件
      </v-card-title>

      <v-card-text>
        <div class="text-body-2 mb-2">
          <span class="font-weight-medium">文件名：</span>
          <span class="text-truncate">{{ fileName }}</span>
        </div>

        <div class="text-body-2">
          <span class="font-weight-medium">进度：</span>
          <span class="text-primary">{{ Math.round(progress) }}%</span>
        </div>
      </v-card-text>

      <v-card-actions>
        <v-spacer />
        <v-btn color="grey" variant="text" @click="$emit('cancel')" :disabled="progress >= 100">
          取消
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * 文件上传进度对话框组件
 */

/**
 * 组件 Props
 */
export interface FileUploadDialogProps {
  /** 对话框显示状态 */
  modelValue: boolean
  /** 上传的文件名 */
  fileName: string
  /** 上传进度 0-100 */
  progress: number
}

/**
 * 组件 Emits
 */
export interface FileUploadDialogEmits {
  /** 更新对话框状态 */
  (e: 'update:modelValue', value: boolean): void
  /** 取消上传 */
  (e: 'cancel'): void
}

withDefaults(defineProps<FileUploadDialogProps>(), {
  modelValue: false,
  fileName: '',
  progress: 0,
})

defineEmits<FileUploadDialogEmits>()
</script>

<style scoped>
.text-truncate {
  display: inline-block;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}
</style>
