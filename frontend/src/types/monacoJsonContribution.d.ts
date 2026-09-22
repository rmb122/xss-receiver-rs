declare module 'monaco-editor/esm/vs/language/json/monaco.contribution' {
  export interface JsonDiagnosticsOptions {
    validate?: boolean
    schemas?: unknown[]
  }

  export const jsonDefaults: {
    setDiagnosticsOptions(options: JsonDiagnosticsOptions): void
  }
}
