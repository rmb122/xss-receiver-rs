<template>
  <div class="message-toast-container">
    <transition-group name="toast" tag="div">
      <v-card
        v-for="toast in toasts"
        :key="toast.id"
        :color="getColor(toast.type)"
        @click="showDetails(toast)"
        class="message-toast mb-3"
        elevation="6"
      >
        <v-card-text class="d-flex align-center pa-2">
          <v-icon :color="getIconColor(toast.type)" class="mr-2">
            {{ getIcon(toast.type) }}
          </v-icon>

          <div class="flex-grow-1 toast-content">
            {{ toast.message }}
          </div>

          <v-btn icon="mdi-close" size="small" variant="text" @click.stop="removeToast(toast.id)">
          </v-btn>
        </v-card-text>

        <!-- 进度条 -->
        <v-progress-linear
          :model-value="getProgress(toast.id)"
          :color="getProgressColor(toast.type)"
          height="4"
        ></v-progress-linear>
      </v-card>
    </transition-group>

    <!-- 详情对话框 -->
    <v-dialog v-model="dialogVisible" max-width="50vw">
      <v-card>
        <v-card-title class="d-flex justify-space-between align-center">
          <span>消息详情</span>
          <v-btn icon="mdi-close" variant="text" @click="dialogVisible = false"></v-btn>
        </v-card-title>
        <v-card-text>
          <div class="mb-2"><strong>消息：</strong>{{ selectedToast?.message }}</div>
          <div v-if="selectedToast?.details">
            <strong>详情：</strong>
            <pre class="error-details mt-2">{{ selectedToast.details }}</pre>
          </div>
        </v-card-text>
        <v-card-actions>
          <v-spacer></v-spacer>
          <v-btn color="primary" @click="dialogVisible = false">关闭</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { registerToastHandler, type ToastMessage, type ToastMessageUpdateFn } from '@/utils/toast'

interface ToastWithTimer extends ToastMessage {
  progress: number
  startTime?: number
  timerId?: number
}

const toasts = ref<ToastWithTimer[]>([])
const dialogVisible = ref(false)
const selectedToast = ref<ToastMessage | null>(null)
const progressTimer = ref<number | null>(null)

// 添加 Toast
const addToast = (message: ToastMessage): ToastMessageUpdateFn => {
  // 两种情况：duration > 0 正常倒计时
  // duration <= 0, 手动控制进度
  let toast: ToastWithTimer

  if (message.duration > 0) {
    const startTime = Date.now()

    // 设置自动关闭定时器
    const timerId = window.setTimeout(() => {
      removeToast(message.id)
    }, message.duration)

    toast = {
      ...message,
      startTime,
      timerId,
      progress: 0,
    }
    toasts.value.push(toast)
  } else {
    toast = {
      ...message,
      progress: 0,
    }
    toasts.value.push(toast)
  }

  // 如果进度条定时器还未启动，启动它
  if (progressTimer.value === null) {
    startProgressUpdate()
  }

  return (message, progress) => {
    toast.message = message
    toast.progress = progress

    if (toast.progress === 100) {
      removeToast(toast.id)
    }
  }
}

// 移除 Toast
const removeToast = (id: string) => {
  const index = toasts.value.findIndex((t) => t.id === id)
  if (index !== -1) {
    const toast = toasts.value[index]
    if (toast !== undefined) {
      if (toast.timerId !== undefined) {
        clearTimeout(toast.timerId)
      }

      toasts.value.splice(index, 1)

      // 如果没有 toast 了，停止进度更新
      if (toasts.value.length === 0 && progressTimer.value !== null) {
        stopProgressUpdate()
      }
    }
  }
}

// 显示详情
const showDetails = (toast: ToastMessage) => {
  if (toast.details) {
    selectedToast.value = toast
    dialogVisible.value = true
  }
}

// 获取颜色
const getColor = (type: string) => {
  const colors: Record<string, string> = {
    success: 'success',
    error: 'error',
    info: 'info',
    warning: 'warning',
  }
  return colors[type] || 'info'
}

// 获取图标颜色
const getIconColor = (_type: string) => {
  return 'white'
}

// 获取进度条颜色
const getProgressColor = (type: string) => {
  const colors: Record<string, string> = {
    success: 'success-darken-2',
    error: 'error-darken-2',
    info: 'info-darken-2',
    warning: 'warning-darken-2',
  }
  return colors[type] || 'info-darken-2'
}

// 获取图标
const getIcon = (type: string) => {
  const icons: Record<string, string> = {
    success: 'mdi-check-circle',
    error: 'mdi-alert-circle',
    info: 'mdi-information',
    warning: 'mdi-alert',
  }
  return icons[type] || 'mdi-information'
}

// 启动进度更新
const startProgressUpdate = () => {
  progressTimer.value = window.setInterval(() => {
    const now = Date.now()
    toasts.value.forEach((toast) => {
      if (toast.startTime !== undefined) {
        const elapsed = now - toast.startTime
        const duration = toast.duration
        const progress = (elapsed / duration) * 100
        toast.progress = Math.max(0, Math.min(100, progress))
      }
    })
  }, 128) // 每 128ms 更新一次
}

// 停止进度更新
const stopProgressUpdate = () => {
  if (progressTimer.value !== null) {
    clearInterval(progressTimer.value)
    progressTimer.value = null
  }
}

// 获取进度
const getProgress = (id: string) => {
  const toast = toasts.value.find((t) => t.id === id)
  return toast?.progress ?? 100
}

onMounted(() => {
  registerToastHandler(addToast)
})

onBeforeUnmount(() => {
  // 清理所有定时器
  toasts.value.forEach((toast) => {
    if (toast.timerId !== undefined) {
      clearTimeout(toast.timerId)
    }
  })
  stopProgressUpdate()
})
</script>

<style scoped>
.message-toast-container {
  position: fixed;
  top: 80px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 9999;
  pointer-events: none;
}

.message-toast {
  position: relative;
  width: 400px;
  pointer-events: all;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  cursor: pointer;
}

.toast-content {
  cursor: pointer;
  word-break: break-word;
  font-size: 0.95rem;
  line-height: 1.4;
}

.error-details {
  white-space: pre-wrap;
  word-break: break-word;
  background-color: #f5f5f5;
  padding: 16px;
  border-radius: 4px;
  max-height: 60vh;
  overflow-y: auto;
}

/* 进入动画 */
.toast-enter-active {
  transition: opacity 0.3s ease;
}

/* 离开动画 */
.toast-leave-active {
  transition: opacity 0.3s ease;
}

/* 进入的初始状态 */
.toast-enter-from {
  opacity: 0;
}

/* 离开的最终状态 */
.toast-leave-to {
  opacity: 0;
}

/* 移动动画 */
.toast-move {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
</style>
