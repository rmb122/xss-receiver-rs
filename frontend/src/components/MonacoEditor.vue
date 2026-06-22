<template>
  <div ref="editorContainer" :style="{ height: height, width: '100%' }"></div>
  <v-dialog v-model="encodingDialog" max-width="360px">
    <v-card>
      <v-card-title>切换编码</v-card-title>
      <v-card-text class="pa-0">
        <v-list density="compact">
          <v-list-item
            v-for="enc in SUPPORTED_ENCODINGS"
            :key="enc"
            :active="enc === encoding"
            @click="selectEncoding(enc)"
          >
            <v-list-item-title>{{ enc }}</v-list-item-title>
          </v-list-item>
        </v-list>
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { monaco } from '@/monaco'
import { typescript, type IDisposable } from 'monaco-editor'
import { scriptEngineTypes } from '@/script-engine-types'
import { SUPPORTED_ENCODINGS } from '@/utils/encoding'

const props = withDefaults(
  defineProps<{
    modelValue?: string
    model?: monaco.editor.ITextModel | null
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
    model: null,
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

const editorContainer = ref<HTMLElement | null>(null)
const encodingDialog = ref(false)
let editor: monaco.editor.IStandaloneCodeEditor | null = null
let internalModel: monaco.editor.ITextModel | null = null
let extraLibDisposable: IDisposable | null = null
let encodingActionDisposable: IDisposable | null = null
let suppressEmit = false

function selectEncoding(enc: string) {
  encodingDialog.value = false
  if (enc !== props.encoding) {
    emit('update:encoding', enc)
  }
}

function activeFilename(): string {
  if (props.model) {
    const path = props.model.uri.path
    return path.startsWith('/') ? path.slice(1) : path
  }
  return props.filename
}

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

function createInternalModel(): monaco.editor.ITextModel {
  const lang = getModelLanguage(props.language, props.filename)
  return monaco.editor.createModel(props.modelValue, lang)
}

function resolveModel(): monaco.editor.ITextModel {
  if (props.model) return props.model
  if (!internalModel) {
    internalModel = createInternalModel()
  }
  return internalModel
}

function registerEncodingAction() {
  if (encodingActionDisposable) {
    encodingActionDisposable.dispose()
    encodingActionDisposable = null
  }
  if (!editor || props.hideEncodingAction) return
  encodingActionDisposable = editor.addAction({
    id: 'switch-encoding',
    label: `切换编码 (当前: ${props.encoding})`,
    contextMenuGroupId: '9_encoding',
    contextMenuOrder: 1,
    run: () => {
      encodingDialog.value = true
    },
  })
}

onMounted(() => {
  if (!editorContainer.value) return

  updateExtraLib(activeFilename())

  editor = monaco.editor.create(editorContainer.value, {
    model: resolveModel(),
    theme: 'vs',
    readOnly: props.readOnly,
    wordWrap: props.wrapLine ? 'on' : 'off',
    automaticLayout: true,
    fontSize: 14,
    padding: {
      top: 3,
    },
  })

  editor.onDidChangeModelContent(() => {
    // Only the internal-model case owns the text value via v-model. When an
    // external model is supplied the parent tracks changes on the model itself.
    if (editor && !props.model && !suppressEmit) {
      emit('update:modelValue', editor.getValue())
    }
  })

  registerEncodingAction()
})

watch(
  () => props.model,
  (newModel) => {
    if (!editor) return
    editor.setModel(newModel ?? resolveModel())
    updateExtraLib(activeFilename())
  },
)

watch(
  () => props.modelValue,
  (newVal) => {
    if (!editor || props.model) return
    if (editor.getValue() !== newVal) {
      suppressEmit = true
      editor.setValue(newVal)
      suppressEmit = false
    }
  },
)

watch(
  () => props.encoding,
  () => {
    registerEncodingAction()
  },
)

watch(
  () => props.readOnly,
  (readOnly) => {
    editor?.updateOptions({ readOnly })
  },
)

watch([() => props.language, () => props.filename], ([newLang, newFilename]) => {
  if (props.model) return
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
  if (encodingActionDisposable) {
    encodingActionDisposable.dispose()
    encodingActionDisposable = null
  }
  if (extraLibDisposable) {
    extraLibDisposable.dispose()
    extraLibDisposable = null
  }
  if (editor) {
    editor.dispose()
    editor = null
  }
  if (internalModel) {
    internalModel.dispose()
    internalModel = null
  }
})

defineExpose({
  getEditor: () => editor,
})
</script>
