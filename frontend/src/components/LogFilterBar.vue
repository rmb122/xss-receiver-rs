<template>
  <div v-if="visible || loadError" class="px-4 pb-4">
    <div v-if="visible" class="pa-1" />
    <form v-if="visible" class="d-flex align-start ga-2 flex-wrap" @submit.prevent="emit('apply')">
      <v-text-field
        v-model="model"
        label="过滤表达式"
        :placeholder="example"
        :error-messages="error"
        hide-details="auto"
        variant="outlined"
        density="compact"
        class="filter-input"
        spellcheck="false"
        autocomplete="off"
      >
        <template #append-inner>
          <v-menu location="bottom end" :close-on-content-click="false" max-width="640">
            <template #activator="{ props: menuProps }">
              <v-btn
                v-bind="menuProps"
                type="button"
                icon="mdi-help-circle-outline"
                variant="text"
                size="small"
                aria-label="过滤语法与字段"
              />
            </template>
            <v-card max-height="480" class="overflow-y-auto">
              <v-card-title>过滤语法与字段</v-card-title>
              <v-card-text class="filter-help d-flex flex-column ga-2">
                <p>
                  <code>{{ example }}</code>
                </p>
                <p>
                  字段: <code>{{ fields }}</code>
                </p>
                <p>
                  字符串使用单引号或双引号, 支持 JSON 转义和 <code>\'</code>, 例如
                  <code>'\u4e2d\u6587'</code> 等同于 <code>"中文"</code>. 数字不加引号.
                  字段名和值区分大小写.
                </p>
                <p>
                  数字和时间支持 <code>{{ '= != < <= > >=' }}</code
                  >. 文本支持 <code>= != contains(field, "text")</code>, contains 按字面包含匹配.
                </p>
                <p>
                  使用 <code>{{ '&& || !' }}</code> 和括号组合条件, 优先级为
                  <code>{{ '! > && > ||' }}</code
                  >.
                </p>
                <p v-if="kind === 'http'">
                  parsed_body_type 仅支持 <code>= !=</code>, 值为
                  <code>"NONE" "FAILED" "FORM" "JSON"</code>.
                </p>
                <p v-if="kind === 'http'">
                  raw_body 支持 <code>= != contains</code>, 将字符串编码为 UTF-8
                  字节后匹配原始请求体. <code>raw_body = "asd"</code> 比较完整内容,
                  <code>contains(raw_body, "asd")</code> 搜索字节子串,
                  <code>contains(raw_body, "\u0000")</code> 搜索 NUL 字节.
                </p>
                <p>
                  时间支持 <code>"2026-09-19"</code>, <code>"2026-09-19 12:00:00"</code> 或
                  <code>"2026-09-19T12:00:00+08:00"</code>. 仅日期表示零点, 日期与时间之间也可用 T,
                  秒后可带小数. 夏令时重叠或跳过的当地时间需要显式偏移.
                </p>
                <p>
                  <code>error_log = null</code> 表示没有错误,
                  <code>error_log != null</code> 表示有错误. 其他文本比较及其取反不匹配空值.
                </p>
              </v-card-text>
            </v-card>
          </v-menu>
        </template>
      </v-text-field>
      <v-btn type="submit" color="primary" :loading="applying">应用</v-btn>
      <v-btn type="button" variant="tonal" @click="emit('clear')">清空</v-btn>
    </form>
    <v-alert v-if="loadError" type="error" variant="tonal" density="compact" class="mt-2">
      {{ loadError }}. 正在显示上次成功加载的结果.
    </v-alert>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const model = defineModel<string>({ required: true })
const props = defineProps<{
  kind: 'http' | 'dns'
  visible: boolean
  error: string
  loadError: string
  applying: boolean
}>()
const emit = defineEmits<{ apply: []; clear: [] }>()
const fields = computed(
  () =>
    'id, client_ip, client_port, location, create_time, error_log, ' +
    (props.kind === 'http'
      ? 'method, path, raw_query, raw_body, parsed_body_type'
      : 'query_name, query_type, query_class'),
)
const example = computed(() =>
  props.kind === 'http'
    ? 'client_ip = "192.0.2.1" && create_time < "2026-09-19 12:00:00"'
    : 'query_type = "A" && contains(query_name, "example.com")',
)
</script>

<style scoped>
.filter-input {
  flex: 1 1 320px;
  min-width: 0;
}

.filter-input :deep(input),
code {
  font-family: 'IBM Plex Mono', monospace;
}

.filter-help {
  overflow-wrap: anywhere;
}
</style>
