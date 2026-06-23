<template>
  <MonacoRawEditor
    ref="rawEditor"
    :model="internalModel"
    :encoding="encoding"
    :height="height"
    :read-only="readOnly"
    :wrap-line="wrapLine"
    :hide-encoding-action="hideEncodingAction"
    @content-change="handleContentChange"
    @update:encoding="$emit('update:encoding', $event)"
  />
</template>

<script setup lang="ts">
import { computed, markRaw, nextTick, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { monaco } from '@/monaco'
import MonacoRawEditor from '@/components/MonacoRawEditor.vue'

const props = withDefaults(
  defineProps<{
    modelValue?: string
    encoding?: string
    language?: string
    filename?: string
    height?: string
    readOnly?: boolean
    wrapLine?: boolean
    hideEncodingAction?: boolean
  }>(),
  {
    modelValue: '',
    encoding: 'UTF-8',
    language: '',
    filename: '',
    height: '500px',
    readOnly: false,
    wrapLine: false,
    hideEncodingAction: false,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'update:encoding': [value: string]
}>()

const rawEditor = ref<InstanceType<typeof MonacoRawEditor>>()
const suppressEmit = ref(false)

function containsExtension(filename: string): boolean {
  const parts = filename.split(/\\|\//g)
  const last = parts[parts.length - 1]!
  const dotIndex = last.lastIndexOf('.')
  return dotIndex > 0 && dotIndex < last.length - 1
}

function getModelLanguage(language?: string, filename?: string): string {
  if (language) {
    return language
  }

  if (filename && containsExtension(filename)) {
    const model = monaco.editor.createModel('', undefined, monaco.Uri.file(filename))
    const detectedLanguage = model.getLanguageId()
    model.dispose()
    return detectedLanguage
  }

  return 'plaintext'
}

function createInternalModel(value = props.modelValue): monaco.editor.ITextModel {
  const lang = getModelLanguage(props.language, props.filename)
  const filename = props.filename || 'model'
  const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
  const uri = monaco.Uri.from({
    scheme: 'inmemory',
    authority: 'monaco-editor',
    path: `/${id}/${filename}`,
  })
  return markRaw(monaco.editor.createModel(value, lang, uri))
}

const internalModel = shallowRef<monaco.editor.ITextModel>(createInternalModel())

const modelLanguage = computed(() => getModelLanguage(props.language, props.filename))

function handleContentChange(model: monaco.editor.ITextModel) {
  if (!suppressEmit.value && model === internalModel.value) {
    emit('update:modelValue', model.getValue())
  }
}

watch(
  () => props.modelValue,
  (newVal) => {
    if (internalModel.value.getValue() !== newVal) {
      suppressEmit.value = true
      try {
        internalModel.value.setValue(newVal)
      } finally {
        suppressEmit.value = false
      }
    }
  },
)

watch(modelLanguage, (language) => {
  monaco.editor.setModelLanguage(internalModel.value, language)
})

watch(
  () => props.filename,
  () => {
    const oldModel = internalModel.value
    internalModel.value = createInternalModel(oldModel.getValue())
    void nextTick(() => {
      oldModel.dispose()
    })
  },
)

onBeforeUnmount(() => {
  internalModel.value.dispose()
})

defineExpose({
  getEditor: () => rawEditor.value?.getEditor() ?? null,
})
</script>
