<template>
  <div class="message-toast-container" aria-label="通知区域">
    <transition-group
      name="toast"
      tag="div"
      :class="['message-toast-list', { 'message-toast-list--expanded': isStackExpanded }]"
      :style="toastListStyle"
      @mouseenter="pointerInStack = true"
      @mouseleave="pointerInStack = false"
      @focusin="focusInStack = true"
      @focusout="handleStackFocusOut"
    >
      <div
        v-for="(toast, index) in toasts"
        :key="toast.id"
        :ref="(element) => setToastElement(toast.id, element)"
        :class="[
          'message-toast-item',
          {
            'message-toast-item--active': isStackExpanded || index === 0,
            'message-toast-item--stacked': !isStackExpanded && index > 0,
            'message-toast-item--hidden': !isStackExpanded && index >= visibleStackCount,
          },
        ]"
        :style="getToastItemStyle(index)"
        :inert="!isStackExpanded && index > 0 ? true : undefined"
        :aria-hidden="!isStackExpanded && index > 0 ? 'true' : undefined"
      >
        <v-card
          :class="[
            'message-toast',
            `message-toast--${toast.type}`,
            { 'message-toast--interactive': toast.details },
          ]"
          :role="getToastRole(toast.type)"
          :aria-label="getToastAriaLabel(toast)"
          :aria-live="toast.type === 'error' ? 'assertive' : 'polite'"
          aria-atomic="true"
          variant="flat"
        >
          <div class="message-toast__body">
            <component
              :is="toast.details ? 'button' : 'div'"
              :class="[
                'message-toast__main',
                { 'message-toast__main--interactive': toast.details },
              ]"
              :type="toast.details ? 'button' : undefined"
              :aria-label="toast.details ? getToastDetailsAriaLabel(toast) : undefined"
              :aria-haspopup="toast.details ? 'dialog' : undefined"
              @click="showDetails(toast)"
            >
              <span class="message-toast__icon" aria-hidden="true">
                <v-icon :icon="getIcon(toast.type)" size="20" />
              </span>

              <span class="message-toast__content">
                {{ toast.message }}
              </span>
            </component>

            <v-btn
              class="message-toast__close"
              icon="mdi-close"
              size="x-small"
              variant="text"
              aria-label="关闭通知"
              title="关闭通知"
              @click.stop="removeToast(toast.id)"
            />
          </div>

          <v-progress-linear
            class="message-toast__progress"
            :model-value="getProgress(toast.id)"
            height="2"
          />
        </v-card>
      </div>
    </transition-group>

    <v-dialog v-model="dialogVisible" max-width="640" width="calc(100% - 32px)" z-index="10000">
      <v-card
        :class="[
          'message-details-card',
          selectedToast ? `message-toast--${selectedToast.type}` : undefined,
        ]"
        rounded="lg"
      >
        <v-card-title class="message-details-title d-flex justify-space-between align-center">
          <span class="d-flex align-center">
            <span class="message-details-icon" aria-hidden="true">
              <v-icon :icon="getIcon(selectedToast?.type ?? 'info')" size="19" />
            </span>
            <span>消息详情</span>
          </span>
          <v-btn
            icon="mdi-close"
            size="small"
            variant="text"
            aria-label="关闭消息详情"
            @click="dialogVisible = false"
          />
        </v-card-title>
        <v-card-text class="message-details-content">
          <div class="message-details-section">
            <div class="message-details-label">消息</div>
            <div class="message-details-message">{{ selectedToast?.message }}</div>
          </div>
          <div v-if="selectedToast?.details" class="message-details-section">
            <div class="message-details-label">详情</div>
            <pre class="error-details">{{ selectedToast.details }}</pre>
          </div>
        </v-card-text>
        <v-card-actions class="message-details-actions">
          <v-spacer />
          <v-btn color="primary" variant="flat" @click="dialogVisible = false">关闭</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import {
  computed,
  reactive,
  ref,
  onMounted,
  onBeforeUnmount,
  type ComponentPublicInstance,
} from 'vue'
import { useMediaQuery } from '@vueuse/core'
import {
  registerToastHandler,
  type ToastMessage,
  type ToastMessageUpdateFn,
  type ToastType,
} from '@/utils/toast'

interface ToastWithTimer extends ToastMessage {
  progress: number
  startTime?: number
  timerId?: number
}

const toasts = ref<ToastWithTimer[]>([])
const dialogVisible = ref(false)
const selectedToast = ref<ToastMessage | null>(null)
const progressTimer = ref<number | null>(null)
const pointerInStack = ref(false)
const focusInStack = ref(false)
const expandStackForViewport = useMediaQuery('(max-width: 600px)')
const toastHeights = reactive<Record<string, number>>({})
const toastElements = new Map<string, HTMLElement>()
const toastElementIds = new Map<HTMLElement, string>()

const visibleStackCount = 3
const stackOffset = 8
const toastGap = 12
const defaultToastHeight = 60

let toastResizeObserver: ResizeObserver | null = null

const isStackExpanded = computed(
  () => expandStackForViewport.value || pointerInStack.value || focusInStack.value,
)

const getToastHeight = (toast: ToastMessage) => toastHeights[toast.id] ?? defaultToastHeight

const frontToastHeight = computed(() => {
  const frontToast = toasts.value[0]
  return frontToast ? getToastHeight(frontToast) : 0
})

const getExpandedOffset = (index: number) => {
  return toasts.value.slice(0, index).reduce((offset, toast) => {
    return offset + getToastHeight(toast) + toastGap
  }, 0)
}

const toastListHeight = computed(() => {
  if (toasts.value.length === 0) return 0

  if (isStackExpanded.value) {
    return toasts.value.reduce((height, toast, index) => {
      return height + getToastHeight(toast) + (index === 0 ? 0 : toastGap)
    }, 0)
  }

  const visibleBackToasts = Math.min(toasts.value.length - 1, visibleStackCount - 1)
  return frontToastHeight.value + visibleBackToasts * stackOffset
})

const toastListStyle = computed<Record<string, string>>(() => ({
  height: `${toastListHeight.value}px`,
  '--front-toast-height': `${frontToastHeight.value}px`,
}))

const getToastItemStyle = (index: number): Record<string, string> => {
  const stackIndex = Math.min(index, visibleStackCount - 1)
  const offset = isStackExpanded.value ? getExpandedOffset(index) : stackIndex * stackOffset
  const scale = isStackExpanded.value ? 1 : 1 - stackIndex * 0.035
  const isVisible = isStackExpanded.value || index < visibleStackCount

  return {
    '--toast-y': `${offset}px`,
    '--toast-scale': `${scale}`,
    '--toast-opacity': isVisible ? '1' : '0',
    '--toast-z-index': `${toasts.value.length - index}`,
  }
}

const setToastElement = (id: string, element: Element | ComponentPublicInstance | null) => {
  if (element === null) {
    const existingElement = toastElements.get(id)
    if (existingElement) {
      toastResizeObserver?.unobserve(existingElement)
      toastElementIds.delete(existingElement)
      toastElements.delete(id)
    }
    if (!toasts.value.some((toast) => toast.id === id)) {
      delete toastHeights[id]
    }
    return
  }

  const itemElement =
    element instanceof Element ? (element as HTMLElement) : (element.$el as HTMLElement)
  const toastElement = itemElement.querySelector<HTMLElement>('.message-toast')
  if (!toastElement || toastElements.get(id) === toastElement) return

  const existingElement = toastElements.get(id)
  if (existingElement) {
    toastResizeObserver?.unobserve(existingElement)
    toastElementIds.delete(existingElement)
  }

  toastElements.set(id, toastElement)
  toastElementIds.set(toastElement, id)
  toastHeights[id] = toastElement.offsetHeight
  toastResizeObserver?.observe(toastElement)
}

const handleStackFocusOut = (event: FocusEvent) => {
  const listElement = event.currentTarget
  const nextTarget = event.relatedTarget

  focusInStack.value =
    listElement instanceof HTMLElement &&
    nextTarget instanceof Node &&
    listElement.contains(nextTarget)
}

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
  } else {
    toast = {
      ...message,
      progress: 0,
    }
  }

  // 右上角堆叠时让最新消息始终最容易看到
  toasts.value.unshift(toast)

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

// 获取图标
const icons: Record<ToastType, string> = {
  success: 'mdi-check-circle',
  error: 'mdi-alert-circle',
  info: 'mdi-information',
  warning: 'mdi-alert',
}

const typeLabels: Record<ToastType, string> = {
  success: '成功',
  error: '错误',
  info: '提示',
  warning: '警告',
}

const getIcon = (type: ToastType) => icons[type]

const getToastRole = (type: ToastType) => (type === 'error' ? 'alert' : 'status')

const getToastAriaLabel = (toast: ToastMessage) => `${typeLabels[toast.type]}：${toast.message}`

const getToastDetailsAriaLabel = (toast: ToastMessage) =>
  `查看${typeLabels[toast.type]}消息详情：${toast.message}`

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
  toastResizeObserver = new ResizeObserver((entries) => {
    entries.forEach((entry) => {
      const element = entry.target as HTMLElement
      const id = toastElementIds.get(element)
      if (id) {
        toastHeights[id] = element.offsetHeight
      }
    })
  })

  toastElements.forEach((element) => toastResizeObserver?.observe(element))
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
  toastResizeObserver?.disconnect()
  toastElements.clear()
  toastElementIds.clear()
})
</script>

<style scoped>
.message-toast-container {
  position: fixed;
  top: calc(var(--v-layout-top, 0px) + 70px);
  right: 24px;
  width: min(360px, calc(100vw - 48px));
  z-index: 9999;
  pointer-events: none;
  transition: top 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.message-toast-list {
  position: relative;
  isolation: isolate;
  width: 100%;
  pointer-events: auto;
  transition: height 0.28s cubic-bezier(0.22, 1, 0.36, 1);
}

.message-toast-item {
  position: absolute;
  top: 0;
  right: 0;
  width: 100%;
  z-index: var(--toast-z-index);
  opacity: var(--toast-opacity);
  transform: translateY(var(--toast-y)) scale(var(--toast-scale));
  transform-origin: top right;
  pointer-events: none;
  transition:
    opacity 0.22s ease,
    transform 0.28s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: transform, opacity;
}

.message-toast-item--stacked {
  max-height: var(--front-toast-height);
  overflow: hidden;
}

.message-toast-item--hidden {
  visibility: hidden;
}

.message-toast-item--active {
  pointer-events: auto;
}

.message-toast-item--active .message-toast {
  pointer-events: auto;
}

.message-toast-item--stacked .message-toast__body,
.message-toast-item--stacked .message-toast__progress {
  opacity: 0;
}

.message-toast {
  --toast-accent: #55738a;
  --toast-background: #ffffff;
  --toast-hover-background: #ffffff;
  --toast-border: #dce3e8;
  --toast-icon-background: #e8eef2;
  --toast-progress-background: #e8eef2;

  position: relative;
  width: 100%;
  overflow: hidden;
  color: #273444;
  background: var(--toast-background);
  border: 1px solid var(--toast-border);
  border-radius: 10px;
  box-shadow:
    0 10px 28px rgba(15, 23, 42, 0.12),
    0 2px 7px rgba(15, 23, 42, 0.08);
  pointer-events: none;
  transition:
    background-color 0.16s ease,
    border-color 0.16s ease,
    box-shadow 0.16s ease;
}

.message-toast--success {
  --toast-accent: #52715d;
  --toast-border: #dce5df;
  --toast-icon-background: #e8f0eb;
  --toast-progress-background: #e8f0eb;
}

.message-toast--error {
  --toast-accent: #9a5b5b;
  --toast-border: #e8dddd;
  --toast-icon-background: #f1e8e8;
  --toast-progress-background: #f1e8e8;
}

.message-toast--info {
  --toast-accent: #55738a;
  --toast-border: #dce3e8;
  --toast-icon-background: #e8eef2;
  --toast-progress-background: #e8eef2;
}

.message-toast--warning {
  --toast-accent: #8b7048;
  --toast-border: #e7e0d3;
  --toast-icon-background: #f1ece2;
  --toast-progress-background: #f1ece2;
}

.message-toast--interactive {
  cursor: pointer;
}

.message-toast--interactive:hover {
  background: var(--toast-hover-background);
  border-color: var(--toast-accent);
  box-shadow:
    0 13px 32px rgba(15, 23, 42, 0.14),
    0 3px 9px rgba(15, 23, 42, 0.09);
}

.message-toast:focus-visible {
  outline: 2px solid var(--toast-accent);
  outline-offset: 2px;
}

.message-toast__body {
  display: flex;
  min-height: 56px;
  align-items: center;
  gap: 11px;
  padding: 12px 10px 12px 13px;
  transition: opacity 0.16s ease;
}

.message-toast__main {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  align-items: center;
  gap: 11px;
  padding: 0;
  color: inherit;
  background: transparent;
  border: 0;
  border-radius: 7px;
  font: inherit;
  text-align: left;
}

.message-toast__main--interactive {
  cursor: pointer;
}

.message-toast__main--interactive:focus-visible {
  outline: 2px solid var(--toast-accent);
  outline-offset: 3px;
}

.message-toast__icon,
.message-details-icon {
  display: inline-flex;
  width: 32px;
  height: 32px;
  flex: 0 0 32px;
  align-items: center;
  justify-content: center;
  color: var(--toast-accent);
  background: var(--toast-icon-background);
  border-radius: 9px;
}

.message-toast__content {
  min-width: 0;
  flex: 1 1 auto;
  align-self: center;
  overflow-wrap: anywhere;
  color: #273444;
  font-size: 0.875rem;
  font-weight: 500;
  line-height: 1.5;
}

.message-toast__close {
  width: 28px !important;
  min-width: 28px !important;
  height: 28px !important;
  flex: 0 0 28px;
  color: #64748b;
  border-radius: 999px;
}

.message-toast__close:hover {
  color: #334155;
  background: var(--toast-icon-background);
}

.message-toast__close:focus-visible {
  outline: 2px solid var(--toast-accent);
  outline-offset: 1px;
}

.message-toast__progress {
  color: var(--toast-accent);
  transition: opacity 0.16s ease;
}

.message-toast__progress :deep(.v-progress-linear__background) {
  background: var(--toast-progress-background) !important;
  opacity: 1 !important;
}

.message-toast__progress :deep(.v-progress-linear__determinate) {
  background: var(--toast-accent) !important;
}

.message-details-card {
  overflow: hidden;
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  box-shadow:
    0 24px 64px rgba(15, 23, 42, 0.18),
    0 8px 24px rgba(15, 23, 42, 0.1);
}

.message-details-title {
  min-height: 64px;
  gap: 16px;
  padding: 14px 18px;
  border-bottom: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  font-size: 1rem;
  font-weight: 600;
}

.message-details-title > span {
  gap: 10px;
}

.message-details-icon {
  width: 30px;
  height: 30px;
  flex-basis: 30px;
  border-radius: 8px;
}

.message-details-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding: 20px !important;
}

.message-details-section {
  min-width: 0;
}

.message-details-label {
  margin-bottom: 7px;
  color: rgba(var(--v-theme-on-surface), 0.56);
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.04em;
}

.message-details-message {
  overflow-wrap: anywhere;
  color: rgba(var(--v-theme-on-surface), 0.9);
  font-size: 0.9rem;
  line-height: 1.6;
}

.error-details {
  max-height: 60vh;
  margin: 0;
  padding: 14px 16px;
  overflow-y: auto;
  color: rgba(var(--v-theme-on-surface), 0.84);
  background: rgba(var(--v-theme-on-surface), 0.045);
  border: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
  border-radius: 8px;
  font-size: 0.82rem;
  line-height: 1.6;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.message-details-actions {
  min-height: 60px;
  padding: 10px 16px;
  border-top: 1px solid rgba(var(--v-border-color), var(--v-border-opacity));
}

.toast-enter-active,
.toast-leave-active {
  transition:
    opacity 0.22s ease,
    transform 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(20px) translateY(var(--toast-y)) scale(var(--toast-scale));
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(16px) translateY(var(--toast-y)) scale(var(--toast-scale));
}

.toast-leave-active {
  z-index: 0;
  pointer-events: none;
}

.toast-move {
  transition: transform 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}

@media (max-width: 600px) {
  .message-toast-container {
    top: calc(var(--v-layout-top, 0px) + 12px);
    right: 12px;
    left: 12px;
    width: auto;
  }

  .message-toast__body {
    padding-inline: 11px 9px;
  }

  .message-details-content {
    padding: 16px !important;
  }
}

@media (prefers-reduced-motion: reduce) {
  .message-toast-container,
  .message-toast-list,
  .message-toast-item,
  .message-toast,
  .message-toast__body,
  .message-toast__progress,
  .toast-enter-active,
  .toast-leave-active,
  .toast-move,
  .message-toast__progress :deep(.v-progress-linear__determinate) {
    transition: none !important;
  }

  .toast-enter-from,
  .toast-leave-to {
    transform: none;
  }
}
</style>
