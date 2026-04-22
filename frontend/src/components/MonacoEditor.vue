<template>
  <div ref="editorContainer" :style="{ height: height, width: '100%' }"></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { monaco } from '@/monaco'
import { typescript, type IDisposable } from 'monaco-editor'
import { scriptEngineTypes } from '@/script-engine-types'

const props = withDefaults(
  defineProps<{
    modelValue?: string
    language?: string
    filename?: string
    height?: string
    readOnly?: boolean
    wrapLine?: boolean
  }>(),
  {
    modelValue: '',
    language: '',
    filename: '',
    height: '500px',
    readOnly: false,
    wrapLine: false,
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const editorContainer = ref<HTMLElement | null>(null)
let editor: monaco.editor.IStandaloneCodeEditor | null = null
let extraLibDisposable: IDisposable | null = null

function updateExtraLib(filename: string) {
  if (extraLibDisposable) {
    extraLibDisposable.dispose()
    extraLibDisposable = null
  }
  if (filename.endsWith('.xjs')) {
    extraLibDisposable = typescript.javascriptDefaults.addExtraLib(
      scriptEngineTypes,
      'ts:script-engine.d.ts',
    )
  }
}

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

onMounted(() => {
  if (!editorContainer.value) return

  const lang = getModelLanguage(props.language, props.filename)
  updateExtraLib(props.filename)

  editor = monaco.editor.create(editorContainer.value, {
    value: props.modelValue,
    language: lang,
    theme: 'vs',
    readOnly: props.readOnly,
    wordWrap: props.wrapLine ? 'on' : 'off',
    automaticLayout: true,
    fontSize: 14,
    padding: {
      top: 3
    }
  })

  editor.onDidChangeModelContent(() => {
    if (editor) {
      emit('update:modelValue', editor.getValue())
    }
  })
})

watch(
  () => props.modelValue,
  (newVal) => {
    if (editor && editor.getValue() !== newVal) {
      editor.setValue(newVal)
    }
  },
)

watch([() => props.language, () => props.filename], ([newLang, newFilename]) => {
  if (editor) {
    const model = editor.getModel()
    if (model) {
      const lang = getModelLanguage(newLang, newFilename)
      monaco.editor.setModelLanguage(model, lang)
    }
  }
  updateExtraLib(newFilename ?? '')
})

onBeforeUnmount(() => {
  if (extraLibDisposable) {
    extraLibDisposable.dispose()
    extraLibDisposable = null
  }
  if (editor) {
    editor.dispose()
    editor = null
  }
})
</script>
