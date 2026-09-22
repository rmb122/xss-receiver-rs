import * as monaco from 'monaco-editor'
import { typescript } from 'monaco-editor'

// Keep this list in sync with rquickjs Context::full and the globals registered
// in src/dispatcher/script_engine. rquickjs 0.14 uses QuickJS-NG 0.16.2.
const SCRIPT_RUNTIME_GLOBALS = new Set([
  'AggregateError',
  'Array',
  'ArrayBuffer',
  'AsyncDisposableStack',
  'Atomics',
  'BigInt',
  'BigInt64Array',
  'BigUint64Array',
  'Boolean',
  'DOMException',
  'DataView',
  'Date',
  'DisposableStack',
  'Error',
  'EvalError',
  'FinalizationRegistry',
  'Float16Array',
  'Float32Array',
  'Float64Array',
  'Function',
  'Infinity',
  'Int16Array',
  'Int32Array',
  'Int8Array',
  'InternalError',
  'Iterator',
  'JSON',
  'Map',
  'Math',
  'NaN',
  'Number',
  'Object',
  'Promise',
  'Proxy',
  'RangeError',
  'ReferenceError',
  'Reflect',
  'RegExp',
  'Set',
  'SharedArrayBuffer',
  'String',
  'SuppressedError',
  'Symbol',
  'SyntaxError',
  'TypeError',
  'URIError',
  'Uint16Array',
  'Uint32Array',
  'Uint8Array',
  'Uint8ClampedArray',
  'WeakMap',
  'WeakRef',
  'WeakSet',
  'atob',
  'base64Decode',
  'base64Encode',
  'btoa',
  'cache',
  'decodeURI',
  'decodeURIComponent',
  'encodeURI',
  'encodeURIComponent',
  'escape',
  'eval',
  'globalThis',
  'http',
  'isFinite',
  'isNaN',
  'parseFloat',
  'parseInt',
  'performance',
  'queueMicrotask',
  'request',
  'response',
  'storage',
  'undefined',
  'unescape',
  'urlDecode',
  'urlEncode',
])

interface TextSpan {
  start: number
  length: number
}

interface CompletionEntry {
  name: string
  kind: string
  kindModifiers?: string
  sortText?: string
  replacementSpan?: TextSpan
}

interface CompletionInfo {
  entries: CompletionEntry[]
  isGlobalCompletion?: boolean
  isMemberCompletion?: boolean
}

interface DisplayPart {
  text: string
}

interface JSDocTagInfo {
  name: string
  text?: string | DisplayPart[]
}

interface CompletionDetails {
  name: string
  kind: string
  displayParts?: DisplayPart[]
  documentation?: DisplayPart[]
  tags?: JSDocTagInfo[]
}

interface DefinitionInfo {
  fileName: string
}

interface ScriptCompletionItem extends monaco.languages.CompletionItem {
  uri: monaco.Uri
  position: monaco.IPosition
  offset: number
}

interface MemberRoot {
  name: string
  offset: number
}

const IDENTIFIER = '[A-Za-z_$][\\w$]*'
const MEMBER_CHAIN = new RegExp(
  `(${IDENTIFIER}(?:\\s*(?:\\?\\.|\\.)\\s*${IDENTIFIER})*)\\s*(?:\\?\\.|\\.)\\s*$`,
)

type WorkerFactory = Awaited<ReturnType<typeof typescript.getJavaScriptWorker>>
type JavaScriptWorker = Awaited<ReturnType<WorkerFactory>>

let workerFactoryPromise: ReturnType<typeof typescript.getJavaScriptWorker> | undefined

function isHandlerScript(uri: monaco.Uri): boolean {
  const path = uri.path.toLowerCase()
  return path.endsWith('.hjs') || path.endsWith('.djs')
}

function isAmbientDeclaration(entry: CompletionEntry): boolean {
  return entry.kindModifiers?.split(',').includes('declare') ?? false
}

function filterScriptGlobals(entries: CompletionEntry[]): CompletionEntry[] {
  return entries.filter(
    (entry) => !isAmbientDeclaration(entry) || SCRIPT_RUNTIME_GLOBALS.has(entry.name),
  )
}

function memberRoot(source: string, position: number): MemberRoot | null {
  const prefix = source.slice(0, position)
  const match = MEMBER_CHAIN.exec(prefix)
  if (!match) return null

  const chain = match[1]
  if (!chain) return null

  const names = chain.match(new RegExp(IDENTIFIER, 'g'))
  if (!names?.length) return null

  const firstName = names[0]
  if (!firstName) return null

  const isGlobalThis = firstName === 'globalThis'
  const name = isGlobalThis && names.length > 1 ? names[1] : names[0]
  if (!name) return null

  const chainStart = match.index + match[0].indexOf(chain)
  const relativeOffset = chain.indexOf(name)
  return { name, offset: chainStart + relativeOffset }
}

function isBundledTypeScriptLib(fileName: string): boolean {
  return /(?:^|[\\/])lib\.[^\\/]+\.d\.ts(?:$|[?#])/.test(fileName)
}

async function getWorker(uri: monaco.Uri): Promise<JavaScriptWorker> {
  workerFactoryPromise ??= typescript.getJavaScriptWorker()
  const workerFactory = await workerFactoryPromise
  return workerFactory(uri)
}

async function filterCompletions(
  model: monaco.editor.ITextModel,
  offset: number,
  worker: JavaScriptWorker,
  info: CompletionInfo,
): Promise<CompletionInfo> {
  if (!isHandlerScript(model.uri)) return info

  if (info.isGlobalCompletion) {
    return { ...info, entries: filterScriptGlobals(info.entries) }
  }

  if (!info.isMemberCompletion) return info

  const root = memberRoot(model.getValue(), offset)
  if (!root) return info

  if (root.name === 'globalThis') {
    return { ...info, entries: filterScriptGlobals(info.entries) }
  }

  if (SCRIPT_RUNTIME_GLOBALS.has(root.name)) return info

  const definitions = (await worker.getDefinitionAtPosition(model.uri.toString(), root.offset)) as
    | DefinitionInfo[]
    | undefined
  const isUnavailableRuntimeGlobal =
    !definitions?.length ||
    definitions.every((definition) => isBundledTypeScriptLib(definition.fileName))

  return isUnavailableRuntimeGlobal ? { ...info, entries: [] } : info
}

function completionKind(kind: string): monaco.languages.CompletionItemKind {
  switch (kind) {
    case 'primitive type':
    case 'keyword':
      return monaco.languages.CompletionItemKind.Keyword
    case 'var':
    case 'local var':
      return monaco.languages.CompletionItemKind.Variable
    case 'property':
    case 'getter':
    case 'setter':
      return monaco.languages.CompletionItemKind.Field
    case 'function':
    case 'method':
    case 'construct':
    case 'call':
    case 'index':
      return monaco.languages.CompletionItemKind.Function
    case 'enum':
      return monaco.languages.CompletionItemKind.Enum
    case 'module':
      return monaco.languages.CompletionItemKind.Module
    case 'class':
      return monaco.languages.CompletionItemKind.Class
    case 'interface':
      return monaco.languages.CompletionItemKind.Interface
    case 'warning':
      return monaco.languages.CompletionItemKind.File
    default:
      return monaco.languages.CompletionItemKind.Property
  }
}

function displayPartsToString(parts?: DisplayPart[]): string {
  return parts?.map((part) => part.text).join('') ?? ''
}

function tagToString(tag: JSDocTagInfo): string {
  let label = `*@${tag.name}*`
  if (tag.name === 'param' && Array.isArray(tag.text)) {
    const [parameterName, ...rest] = tag.text
    if (parameterName) label += `\`${parameterName.text}\``
    if (rest.length) label += ` — ${rest.map((part) => part.text).join(' ')}`
  } else if (Array.isArray(tag.text)) {
    label += ` — ${tag.text.map((part) => part.text).join(' ')}`
  } else if (tag.text) {
    label += ` — ${tag.text}`
  }
  return label
}

function completionDocumentation(details: CompletionDetails): string {
  let documentation = displayPartsToString(details.documentation)
  for (const tag of details.tags ?? []) {
    documentation += `\n\n${tagToString(tag)}`
  }
  return documentation
}

export function configureScriptCompletions(): monaco.IDisposable {
  const defaults = typescript.javascriptDefaults
  defaults.setModeConfiguration({
    ...defaults.modeConfiguration,
    completionItems: false,
  })

  return monaco.languages.registerCompletionItemProvider('javascript', {
    triggerCharacters: ['.'],

    async provideCompletionItems(model, position, _context, token) {
      const word = model.getWordUntilPosition(position)
      const wordRange = new monaco.Range(
        position.lineNumber,
        word.startColumn,
        position.lineNumber,
        word.endColumn,
      )
      const offset = model.getOffsetAt(position)
      const worker = await getWorker(model.uri)
      if (token.isCancellationRequested || model.isDisposed()) return

      const rawInfo = (await worker.getCompletionsAtPosition(model.uri.toString(), offset)) as
        | CompletionInfo
        | undefined
      if (!rawInfo || token.isCancellationRequested || model.isDisposed()) return

      const info = await filterCompletions(model, offset, worker, rawInfo)
      if (token.isCancellationRequested || model.isDisposed()) return

      const suggestions: ScriptCompletionItem[] = info.entries.map((entry) => {
        let range: monaco.IRange = wordRange
        if (entry.replacementSpan) {
          const start = model.getPositionAt(entry.replacementSpan.start)
          const end = model.getPositionAt(
            entry.replacementSpan.start + entry.replacementSpan.length,
          )
          range = new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column)
        }

        const tags = entry.kindModifiers?.includes('deprecated')
          ? [monaco.languages.CompletionItemTag.Deprecated]
          : []
        return {
          uri: model.uri,
          position,
          offset,
          range,
          label: entry.name,
          insertText: entry.name,
          sortText: entry.sortText,
          kind: completionKind(entry.kind),
          tags,
        }
      })

      return { suggestions }
    },

    async resolveCompletionItem(item, token) {
      const completion = item as ScriptCompletionItem
      if (token.isCancellationRequested) return completion

      const worker = await getWorker(completion.uri)
      const label = typeof completion.label === 'string' ? completion.label : completion.label.label
      const details = (await worker.getCompletionEntryDetails(
        completion.uri.toString(),
        completion.offset,
        label,
      )) as CompletionDetails | undefined
      if (!details || token.isCancellationRequested) return completion

      return {
        ...completion,
        label: details.name,
        kind: completionKind(details.kind),
        detail: displayPartsToString(details.displayParts),
        documentation: { value: completionDocumentation(details) },
      }
    },
  })
}
