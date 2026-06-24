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
import { dnsScriptEngineTypes, httpScriptEngineTypes } from '@/script-engine-types'
import { SUPPORTED_ENCODINGS } from '@/utils/encoding'

const props = withDefaults(
  defineProps<{
    model: monaco.editor.ITextModel | null
    encoding?: string
    height?: string
    readOnly?: boolean
    wrapLine?: boolean
    hideEncodingAction?: boolean
  }>(),
  {
    encoding: 'UTF-8',
    height: '500px',
    readOnly: false,
    wrapLine: false,
    hideEncodingAction: false,
  },
)

const emit = defineEmits<{
  'update:encoding': [value: string]
  'content-change': [model: monaco.editor.ITextModel]
}>()

const editorContainer = ref<HTMLElement | null>(null)
const encodingDialog = ref(false)
let editor: monaco.editor.IStandaloneCodeEditor | null = null
let extraLibDisposable: IDisposable | null = null
let encodingActionDisposable: IDisposable | null = null

function selectEncoding(enc: string) {
  encodingDialog.value = false
  if (enc !== props.encoding) {
    emit('update:encoding', enc)
  }
}

function modelFilename(model: monaco.editor.ITextModel | null): string {
  if (!model) return ''
  const path = model.uri.path
  return path.startsWith('/') ? path.slice(1) : path
}

function updateExtraLib(filename: string) {
  if (extraLibDisposable) {
    extraLibDisposable.dispose()
    extraLibDisposable = null
  }
  if (filename.endsWith('.hjs')) {
    extraLibDisposable = typescript.javascriptDefaults.addExtraLib(
      httpScriptEngineTypes,
      'ts:http-script-engine.d.ts',
    )
  } else if (filename.endsWith('.djs')) {
    extraLibDisposable = typescript.javascriptDefaults.addExtraLib(
      dnsScriptEngineTypes,
      'ts:dns-script-engine.d.ts',
    )
  }
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

  updateExtraLib(modelFilename(props.model))

  editor = monaco.editor.create(editorContainer.value, {
    model: props.model,
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
    const model = editor?.getModel()
    if (model) emit('content-change', model)
  })

  registerEncodingAction()
})

watch(
  () => props.model,
  (newModel) => {
    if (!editor) return
    editor.setModel(newModel)
    updateExtraLib(modelFilename(newModel))
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
})

defineExpose({
  getEditor: () => editor,
})
</script>
