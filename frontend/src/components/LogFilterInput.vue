<template>
  <v-input
    :model-value="model"
    :error-messages="error"
    :focused="focused"
    hide-details="auto"
    density="compact"
    class="filter-input"
  >
    <v-field
      label="过滤表达式"
      variant="outlined"
      density="compact"
      active
      :dirty="!!model"
      :focused="focused"
      :error="!!error"
    >
      <template #default="{ props: fieldProps }">
        <div
          v-bind="fieldProps"
          class="filter-control"
          @click="editor?.focus()"
          @paste.capture="handlePaste"
        >
          <div ref="container" class="filter-editor" />
        </div>
      </template>
      <template #append-inner>
        <slot name="append-inner" />
      </template>
    </v-field>
  </v-input>
  <!-- Vuetify fields contain layout, so overflowing widgets live outside them. -->
  <Teleport to="body">
    <div ref="widgetsContainer" class="monaco-editor" />
  </Teleport>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { monaco } from '@/monaco'
import { createLogFilterEditor } from '@/log-filter-editor'
import { logFilterLanguageId } from '@/log-filter-language'
import type { LogFilterKind } from '@/log-filter'

const model = defineModel<string>({ required: true })
const props = defineProps<{
  kind: LogFilterKind
  placeholder: string
  error: string
}>()
const emit = defineEmits<{ apply: [] }>()
const container = ref<HTMLElement>()
const widgetsContainer = ref<HTMLElement>()
const focused = ref(false)
let editor: monaco.editor.ICodeEditor | undefined
let textModel: monaco.editor.ITextModel | undefined
const disposables: monaco.IDisposable[] = []

function singleLine(value: string): string {
  return value.replace(/\r\n|\r|\n/g, ' ')
}

function handlePaste(event: ClipboardEvent) {
  const text = event.clipboardData?.getData('text/plain')
  if (!editor || text === undefined || !/[\r\n]/.test(text)) return
  event.preventDefault()
  event.stopPropagation()
  editor.trigger('filter-paste', 'type', { text: singleLine(text) })
}

onMounted(() => {
  if (!container.value) return
  textModel = monaco.editor.createModel(singleLine(model.value), logFilterLanguageId(props.kind))
  editor = createLogFilterEditor(
    container.value,
    {
      ariaLabel: '过滤表达式',
      placeholder: props.placeholder,
      automaticLayout: true,
      fontFamily: "'IBM Plex Mono', monospace",
      fontSize: 14,
      lineHeight: 24,
      padding: { top: 0, bottom: 0 },
      wordWrap: 'off',
      lineNumbers: 'off',
      glyphMargin: false,
      folding: false,
      lineDecorationsWidth: 0,
      lineNumbersMinChars: 0,
      minimap: { enabled: false },
      overviewRulerLanes: 0,
      hideCursorInOverviewRuler: true,
      scrollBeyondLastLine: false,
      scrollbar: {
        vertical: 'hidden',
        horizontal: 'hidden',
        handleMouseWheel: false,
      },
      renderLineHighlight: 'none',
      guides: { indentation: false, bracketPairs: false },
      bracketPairColorization: { enabled: false },
      quickSuggestions: { other: true, comments: false, strings: true },
      suggest: { showWords: false, snippetsPreventQuickSuggestions: false },
      acceptSuggestionOnEnter: 'on',
      tabFocusMode: true,
      fixedOverflowWidgets: true,
      overflowWidgetsDomNode: widgetsContainer.value,
      multiCursorLimit: 1,
      dragAndDrop: false,
      dropIntoEditor: { enabled: false },
    },
    () => emit('apply'),
  )
  editor.setModel(textModel)
  const inputEditor = editor
  disposables.push(
    inputEditor.onDidFocusEditorText(() => {
      focused.value = true
    }),
    inputEditor.onDidBlurEditorText(() => {
      focused.value = false
    }),
    inputEditor.onDidChangeModelContent(() => {
      model.value = inputEditor.getValue()
    }),
    inputEditor.onKeyDown((event) => {
      if (event.keyCode === monaco.KeyCode.Enter) {
        event.preventDefault()
      }
      // Snippets must not take the filter out of the form's normal Tab order.
      if (event.keyCode === monaco.KeyCode.Tab && !inputEditor.inComposition) {
        inputEditor.trigger('filter-input', 'leaveSnippet', {})
      }
    }),
  )
})

watch(model, (value) => {
  const normalized = singleLine(value)
  if (textModel && textModel.getValue() !== normalized) textModel.setValue(normalized)
})
watch(
  () => props.kind,
  (kind) => {
    if (textModel) monaco.editor.setModelLanguage(textModel, logFilterLanguageId(kind))
  },
)
watch(
  () => props.placeholder,
  (placeholder) => editor?.updateOptions({ placeholder }),
)

onBeforeUnmount(() => {
  for (const disposable of disposables) disposable.dispose()
  editor?.dispose()
  textModel?.dispose()
})
</script>

<style scoped>
.filter-input {
  flex: 1 1 320px;
  min-width: 0;
}

.filter-control {
  min-width: 0;
  padding-top: 8px;
  padding-bottom: 8px;
  cursor: text;
}

.filter-editor {
  width: 100%;
  height: 24px;
  min-width: 0;
}
</style>
