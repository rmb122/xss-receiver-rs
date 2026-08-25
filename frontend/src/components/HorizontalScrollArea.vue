<template>
  <div
    class="horizontal-scroll-area"
    :class="{
      'scrollbar-visible': scrollbarVisible && hasOverflow,
      'scrollbar-dragging': scrollbarDragging,
    }"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
  >
    <div ref="viewport" class="scroll-viewport" @scroll="onScroll" @wheel="onWheel">
      <div ref="content" class="scroll-content">
        <slot />
      </div>
    </div>
    <div
      v-show="hasOverflow"
      ref="scrollbarTrack"
      class="scrollbar-track"
      aria-hidden="true"
      @pointerdown="onTrackPointerDown"
    >
      <div
        class="scrollbar-thumb"
        :style="scrollbarThumbStyle"
        @pointerdown.stop="onThumbPointerDown"
        @pointermove="onThumbPointerMove"
        @pointerup="onThumbPointerUp"
        @pointercancel="onThumbPointerUp"
        @lostpointercapture="onThumbLostPointerCapture"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const SCROLLBAR_HIDE_DELAY = 500
const MIN_THUMB_SIZE = 20
const OVERFLOW_EPSILON = 1
const WHEEL_LINE_SIZE = 16

const props = withDefaults(
  defineProps<{
    revealIndex?: number | null
    layoutKey?: string | number
  }>(),
  {
    revealIndex: null,
    layoutKey: '',
  },
)

const viewport = ref<HTMLElement | null>(null)
const content = ref<HTMLElement | null>(null)
const scrollbarTrack = ref<HTMLElement | null>(null)
const hasOverflow = ref(false)
const scrollbarVisible = ref(false)
const scrollbarDragging = ref(false)
const thumbWidth = ref(0)
const thumbOffset = ref(0)

const scrollbarThumbStyle = computed(() => ({
  width: `${thumbWidth.value}px`,
  transform: `translateX(${thumbOffset.value}px)`,
}))

let resizeObserver: ResizeObserver | null = null
let layoutFrame: number | null = null
let revealItemOnLayout = false
let scrollbarHideTimer: number | null = null
let pointerOverArea = false
let draggingPointerId: number | null = null
let dragStartClientX = 0
let dragStartScrollLeft = 0
let preserveScrollPosition = false

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function clearHideTimer() {
  if (scrollbarHideTimer === null) return
  window.clearTimeout(scrollbarHideTimer)
  scrollbarHideTimer = null
}

function hideScrollbarLater() {
  clearHideTimer()
  if (pointerOverArea || scrollbarDragging.value || !hasOverflow.value) return
  scrollbarHideTimer = window.setTimeout(() => {
    scrollbarHideTimer = null
    if (!pointerOverArea && !scrollbarDragging.value) scrollbarVisible.value = false
  }, SCROLLBAR_HIDE_DELAY)
}

function revealScrollbar() {
  if (!hasOverflow.value) return
  clearHideTimer()
  scrollbarVisible.value = true
  hideScrollbarLater()
}

function updateMetrics() {
  const viewportElement = viewport.value
  if (!viewportElement) return

  const viewportWidth = viewportElement.clientWidth
  const scrollWidth = viewportElement.scrollWidth
  const maxScrollLeft = Math.max(0, scrollWidth - viewportWidth)
  const overflowing = maxScrollLeft > OVERFLOW_EPSILON
  hasOverflow.value = overflowing

  if (!overflowing || viewportWidth <= 0) {
    thumbWidth.value = viewportWidth
    thumbOffset.value = 0
    scrollbarVisible.value = false
    clearHideTimer()
    return
  }

  const nextThumbWidth = Math.min(
    viewportWidth,
    Math.max(MIN_THUMB_SIZE, Math.floor((viewportWidth * viewportWidth) / scrollWidth)),
  )
  const thumbTravel = viewportWidth - nextThumbWidth
  thumbWidth.value = nextThumbWidth
  thumbOffset.value =
    maxScrollLeft > 0
      ? clamp(
          Math.round((viewportElement.scrollLeft / maxScrollLeft) * thumbTravel),
          0,
          thumbTravel,
        )
      : 0

  if (pointerOverArea || scrollbarDragging.value) scrollbarVisible.value = true
}

function revealChild(index: number | null) {
  const viewportElement = viewport.value
  const contentElement = content.value
  if (!viewportElement || !contentElement || index === null || index < 0) return

  const child = contentElement.children.item(index) as HTMLElement | null
  if (!child) return

  const viewportLeft = viewportElement.scrollLeft
  const viewportRight = viewportLeft + viewportElement.clientWidth
  const childLeft = child.offsetLeft
  const childRight = childLeft + child.offsetWidth
  const maxScrollLeft = Math.max(0, viewportElement.scrollWidth - viewportElement.clientWidth)

  let nextScrollLeft = viewportLeft
  if (child.offsetWidth > viewportElement.clientWidth || childLeft < viewportLeft) {
    nextScrollLeft = childLeft
  } else if (childRight > viewportRight) {
    nextScrollLeft = childRight - viewportElement.clientWidth
  }

  nextScrollLeft = clamp(nextScrollLeft, 0, maxScrollLeft)
  if (Math.abs(nextScrollLeft - viewportLeft) > OVERFLOW_EPSILON) {
    viewportElement.scrollLeft = nextScrollLeft
  }
}

function scheduleLayout(revealItem = false) {
  revealItemOnLayout ||= revealItem
  if (layoutFrame !== null) return
  layoutFrame = window.requestAnimationFrame(() => {
    layoutFrame = null
    const shouldRevealItem = revealItemOnLayout
    revealItemOnLayout = false
    updateMetrics()
    if (shouldRevealItem) revealChild(props.revealIndex)
    updateMetrics()
  })
}

function onScroll() {
  scheduleLayout()
  revealScrollbar()
}

function normalizeWheelDelta(event: WheelEvent, delta: number, viewportWidth: number) {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) return delta * WHEEL_LINE_SIZE
  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) return delta * viewportWidth
  return delta
}

function onWheel(event: WheelEvent) {
  const viewportElement = viewport.value
  if (!viewportElement || event.ctrlKey || event.metaKey) return

  const maxScrollLeft = Math.max(0, viewportElement.scrollWidth - viewportElement.clientWidth)
  if (maxScrollLeft <= OVERFLOW_EPSILON) return

  const predominantDelta =
    Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY
  const delta = normalizeWheelDelta(event, predominantDelta, viewportElement.clientWidth)
  const nextScrollLeft = clamp(viewportElement.scrollLeft + delta, 0, maxScrollLeft)
  if (Math.abs(nextScrollLeft - viewportElement.scrollLeft) <= OVERFLOW_EPSILON) return

  event.preventDefault()
  event.stopPropagation()
  viewportElement.scrollLeft = nextScrollLeft
  revealScrollbar()
}

function onMouseEnter() {
  pointerOverArea = true
  clearHideTimer()
  if (hasOverflow.value) scrollbarVisible.value = true
}

function onMouseLeave() {
  pointerOverArea = false
  hideScrollbarLater()
}

function onTrackPointerDown(event: PointerEvent) {
  const viewportElement = viewport.value
  const track = scrollbarTrack.value
  if (!viewportElement || !track || event.button !== 0 || !event.isPrimary) return

  updateMetrics()
  const trackRect = track.getBoundingClientRect()
  const thumbTravel = trackRect.width - thumbWidth.value
  const maxScrollLeft = Math.max(0, viewportElement.scrollWidth - viewportElement.clientWidth)
  if (thumbTravel <= 0 || maxScrollLeft <= 0) return

  event.preventDefault()
  const desiredThumbOffset = clamp(
    event.clientX - trackRect.left - thumbWidth.value / 2,
    0,
    thumbTravel,
  )
  viewportElement.scrollLeft = (desiredThumbOffset / thumbTravel) * maxScrollLeft
  revealScrollbar()
}

function onThumbPointerDown(event: PointerEvent) {
  const viewportElement = viewport.value
  if (!viewportElement || event.button !== 0 || !event.isPrimary) return

  event.preventDefault()
  draggingPointerId = event.pointerId
  dragStartClientX = event.clientX
  dragStartScrollLeft = viewportElement.scrollLeft
  scrollbarDragging.value = true
  const thumb = event.currentTarget as HTMLElement
  thumb.setPointerCapture(event.pointerId)
  revealScrollbar()
}

function onThumbPointerMove(event: PointerEvent) {
  const viewportElement = viewport.value
  const track = scrollbarTrack.value
  if (
    !viewportElement ||
    !track ||
    !scrollbarDragging.value ||
    event.pointerId !== draggingPointerId
  ) {
    return
  }

  const maxScrollLeft = Math.max(0, viewportElement.scrollWidth - viewportElement.clientWidth)
  const thumbTravel = track.clientWidth - thumbWidth.value
  if (maxScrollLeft <= 0 || thumbTravel <= 0) return

  const scrollDelta = ((event.clientX - dragStartClientX) / thumbTravel) * maxScrollLeft
  viewportElement.scrollLeft = clamp(dragStartScrollLeft + scrollDelta, 0, maxScrollLeft)
}

function finishThumbDrag() {
  draggingPointerId = null
  scrollbarDragging.value = false
  if (pointerOverArea) {
    scrollbarVisible.value = hasOverflow.value
  } else {
    hideScrollbarLater()
  }
}

function onThumbPointerUp(event: PointerEvent) {
  if (event.pointerId !== draggingPointerId) return
  const thumb = event.currentTarget as HTMLElement
  draggingPointerId = null
  if (thumb.hasPointerCapture(event.pointerId)) thumb.releasePointerCapture(event.pointerId)
  scrollbarDragging.value = false
  if (!pointerOverArea) hideScrollbarLater()
}

function onThumbLostPointerCapture(event: PointerEvent) {
  if (event.pointerId === draggingPointerId) finishThumbDrag()
}

function preserveScrollPositionOnce() {
  preserveScrollPosition = true
}

watch(
  () => [props.revealIndex, props.layoutKey] as const,
  () => {
    const shouldRevealItem = !preserveScrollPosition
    preserveScrollPosition = false
    scheduleLayout(shouldRevealItem)
  },
  { flush: 'post' },
)

onMounted(() => {
  resizeObserver = new ResizeObserver((entries) => {
    const viewportResized = entries.some((entry) => entry.target === viewport.value)
    scheduleLayout(viewportResized)
  })
  if (viewport.value) resizeObserver.observe(viewport.value)
  if (content.value) resizeObserver.observe(content.value)
  scheduleLayout(true)
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  if (layoutFrame !== null) window.cancelAnimationFrame(layoutFrame)
  layoutFrame = null
  revealItemOnLayout = false
  clearHideTimer()
  pointerOverArea = false
  draggingPointerId = null
  scrollbarDragging.value = false
})

defineExpose({ preserveScrollPositionOnce })
</script>

<style scoped>
.horizontal-scroll-area {
  --horizontal-scrollbar-thumb: rgba(100, 100, 100, 0.4);
  --horizontal-scrollbar-thumb-hover: rgba(100, 100, 100, 0.7);
  --horizontal-scrollbar-thumb-active: rgba(0, 0, 0, 0.6);

  position: relative;
  overflow: hidden;
}
.scroll-viewport {
  overflow-x: auto;
  overflow-y: hidden;
  overscroll-behavior-x: contain;
  scrollbar-width: none;
  -ms-overflow-style: none;
  -webkit-overflow-scrolling: touch;
}
.scroll-viewport::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}
.scroll-content {
  display: flex;
  width: max-content;
  min-width: 100%;
}
.scrollbar-track {
  position: absolute;
  z-index: 3;
  right: 0;
  bottom: 0;
  left: 0;
  height: 3px;
  opacity: 0;
  pointer-events: none;
  touch-action: none;
  transition: opacity 800ms linear;
}
.horizontal-scroll-area.scrollbar-visible .scrollbar-track {
  opacity: 1;
  pointer-events: auto;
  transition-duration: 100ms;
}
.scrollbar-thumb {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  background-color: var(--horizontal-scrollbar-thumb);
  touch-action: none;
  will-change: transform;
}
.scrollbar-track:hover .scrollbar-thumb {
  background-color: var(--horizontal-scrollbar-thumb-hover);
}
.horizontal-scroll-area.scrollbar-dragging .scrollbar-thumb {
  background-color: var(--horizontal-scrollbar-thumb-active);
}

@media (prefers-reduced-motion: reduce) {
  .scrollbar-track {
    transition: none;
  }
}
</style>
