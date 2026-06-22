<template>
  <v-dialog
    :model-value="modelValue"
    height="85vh"
    max-width="50vw"
    scrollable
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon class="mr-2">mdi-file-edit</v-icon>
        编辑文件: {{ fileName }}
        <v-spacer />
        <v-btn icon size="small" variant="text" @click="$emit('update:modelValue', false)">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>
      <v-divider />
      <v-card-text class="pa-0">
        <MonacoEditor
          v-if="modelValue"
          v-model="editText"
          :encoding="encoding"
          :filename="fileName"
          height="100%"
          @update:encoding="handleEncodingChange"
        />
      </v-card-text>
      <v-divider />
      <v-card-actions>
        <v-spacer />
        <v-btn variant="text" @click="$emit('update:modelValue', false)">取消</v-btn>
        <v-btn color="primary" variant="flat" :loading="loading" @click="handleSave"> 保存 </v-btn>
        <v-btn color="primary" variant="flat" :loading="loading" @click="handleSaveAndClose">
          保存并关闭
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import MonacoEditor from '@/components/MonacoEditor.vue'
import { decodeBytes, encodeBytes } from '@/utils/encoding'

interface Props {
  modelValue: boolean
  fileName: string
  fileBytes: Uint8Array<ArrayBuffer>
  loading?: boolean
}

interface Emits {
  (e: 'update:modelValue', value: boolean): void
  (e: 'save', bytes: Uint8Array<ArrayBuffer>): void
  (e: 'save-and-close', bytes: Uint8Array<ArrayBuffer>): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const encoding = ref('UTF-8')
const editText = ref('')

watch(
  () => props.fileBytes,
  (newValue) => {
    editText.value = decodeBytes(newValue, encoding.value)
  },
  { immediate: true },
)

function handleEncodingChange(newEncoding: string) {
  encoding.value = newEncoding
  editText.value = decodeBytes(props.fileBytes, newEncoding)
}

function handleSave() {
  emit('save', encodeBytes(editText.value, encoding.value))
}

function handleSaveAndClose() {
  emit('save-and-close', encodeBytes(editText.value, encoding.value))
}
</script>
