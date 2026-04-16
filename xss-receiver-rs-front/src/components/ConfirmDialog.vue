<template>
  <v-dialog v-model="dialog" max-width="400">
    <v-card>
      <v-card-title>{{ title }}</v-card-title>
      <v-card-text>{{ message }}</v-card-text>
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="cancel">取消</v-btn>
        <v-btn color="error" variant="flat" @click="confirm" :loading="loading">确认</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const dialog = ref(false)
const title = ref('')
const message = ref('')
const loading = ref(false)

let resolvePromise: ((value: boolean) => void) | null = null

function open(t: string, msg: string): Promise<boolean> {
  title.value = t
  message.value = msg
  dialog.value = true
  loading.value = false
  return new Promise((resolve) => {
    resolvePromise = resolve
  })
}

function confirm() {
  dialog.value = false
  resolvePromise?.(true)
}

function cancel() {
  dialog.value = false
  resolvePromise?.(false)
}

defineExpose({ open, loading })
</script>
