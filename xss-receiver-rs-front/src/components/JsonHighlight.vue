<template>
  <div class="json-content" :style="{ maxHeight: maxHeight }">
    <JsonValue :value="data" :depth="0" />
  </div>
</template>

<script setup lang="ts">
import JsonValue from './JsonValue.vue'

const props = withDefaults(
  defineProps<{
    data: any
    maxHeight?: string
  }>(),
  {
    maxHeight: '300px',
  },
)
</script>

<style scoped>
.json-content {
  overflow: auto;
  background: #f6f8fa;
  padding: 12px;
  border-radius: 6px;
  border: 1px solid #d0d7de;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.5;
}

/* JSON 语法高亮颜色 */
.json-content :deep(.json-string) {
  color: #22863a;
}

.json-content :deep(.json-number) {
  color: #005cc5;
}

.json-content :deep(.json-boolean) {
  color: #e36209;
}

.json-content :deep(.json-null) {
  color: #d73a49;
}

.json-content :deep(.json-key) {
  color: #032f62;
  font-weight: 600;
}

.json-content :deep(.json-punctuation) {
  color: #24292e;
}

.json-content :deep(.json-error) {
  color: #d73a49;
  font-style: italic;
}

.json-content :deep(.json-other) {
  color: #6a737d;
  font-style: italic;
}
</style>
