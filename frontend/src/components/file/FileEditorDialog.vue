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
          :model-value="editText"
          :encoding="encoding"
          :filename="fileName"
          height="100%"
          @update:model-value="handleTextChange"
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
    <ConfirmDialog ref="confirmDialog" />
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import MonacoEditor from '@/components/MonacoEditor.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
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

const confirmDialog = ref<InstanceType<typeof ConfirmDialog>>()
const encoding = ref('UTF-8')
const editText = ref('')
const dirty = ref(false)

watch(
  () => props.fileBytes,
  (newValue) => {
    editText.value = decodeBytes(newValue, encoding.value)
    dirty.value = false
  },
  { immediate: true },
)

function handleTextChange(value: string) {
  editText.value = value
  dirty.value = true
}

async function handleEncodingChange(newEncoding: string) {
  if (newEncoding === encoding.value) return

  if (dirty.value) {
    const confirmed = await confirmDialog.value!.open(
      '切换编码',
      '切换编码会丢失当前未保存的修改，确定继续吗？',
    )
    if (!confirmed) return
  }

  encoding.value = newEncoding
  editText.value = decodeBytes(props.fileBytes, newEncoding)
  dirty.value = false
}

function handleSave() {
  emit('save', encodeBytes(editText.value, encoding.value))
}

function handleSaveAndClose() {
  emit('save-and-close', encodeBytes(editText.value, encoding.value))
}
</script>
