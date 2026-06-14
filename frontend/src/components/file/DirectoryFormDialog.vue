<template>
  <v-dialog
    :model-value="modelValue"
    @update:model-value="emit('update:modelValue', $event)"
    max-width="500"
  >
    <v-card>
      <v-card-title>{{ title }}</v-card-title>
      <v-card-text>
        <v-text-field
          v-model="value"
          :label="label"
          :placeholder="placeholder"
          variant="outlined"
          density="compact"
          autofocus
          @keyup.enter="handleSubmit"
        />
      </v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="emit('update:modelValue', false)">取消</v-btn>
        <v-btn color="primary" variant="flat" :disabled="!value" @click="handleSubmit">确定</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{
  modelValue: boolean
  title: string
  label: string
  placeholder?: string
  initialValue?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  submit: [value: string]
}>()

const value = ref(props.initialValue || '')

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      value.value = props.initialValue || ''
    }
  },
)

function handleSubmit() {
  if (!value.value) return
  emit('submit', value.value)
}
</script>
