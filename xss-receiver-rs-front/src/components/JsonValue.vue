<template>
  <span class="json-value">
    <!-- null -->
    <span v-if="value === null" class="json-null">null</span>
    <!-- boolean -->
    <span v-else-if="typeof value === 'boolean'" class="json-boolean">
      {{ value }}
    </span>
    <!-- number -->
    <span v-else-if="typeof value === 'number'" class="json-number">
      {{ value }}
    </span>
    <!-- string -->
    <span v-else-if="typeof value === 'string'" class="json-string">"{{ value }}"</span>
    <!-- array -->
    <span v-else-if="Array.isArray(value)">
      <!-- 空数组或单元素数组：内联显示 -->
      <template v-if="value.length <= 1">
        <span class="json-punctuation">[</span>
        <JsonValue v-if="value.length === 1" :value="value[0]" :depth="depth" />
        <span class="json-punctuation">]</span>
      </template>

      <!-- 多元素数组：换行显示 -->
      <template v-else>
        <span class="json-punctuation">[</span>
        <template v-if="value.length > 0">
          <span v-for="(item, index) in value" :key="index">
            <br /><span :style="{ paddingLeft: indent }"></span>
            <JsonValue :value="item" :depth="depth + 1" />
            <span v-if="index < value.length - 1" class="json-punctuation">,</span>
          </span>
          <br /><span :style="{ paddingLeft: parentIndent }"></span>
        </template>
        <span class="json-punctuation">]</span>
      </template>
    </span>
    <!-- object -->
    <span v-else-if="typeof value === 'object' && value !== null">
      <span class="json-punctuation">{</span>
      <template v-if="Object.keys(value).length > 0">
        <span v-for="(key, index) in Object.keys(value)" :key="key">
          <br /><span :style="{ paddingLeft: indent }"></span>
          <span class="json-key">"{{ key }}"</span>
          <span class="json-punctuation">: </span>
          <JsonValue :value="value[key]" :depth="depth + 1" />
          <span v-if="index < Object.keys(value).length - 1" class="json-punctuation">,</span>
        </span>
        <br /><span :style="{ paddingLeft: parentIndent }"></span>
      </template>
      <span class="json-punctuation">}</span>
    </span>
    <!-- 其他类型 -->
    <span v-else class="json-other">{{ String(value) }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  value: any
  depth: number
}>()

// 当前层级的缩进
const indent = computed(() => (props.depth + 1) * 20 + 'px')
// 父层级的缩进
const parentIndent = computed(() => props.depth * 20 + 'px')
</script>

<style scoped>
.json-value {
  display: inline;
}
</style>
