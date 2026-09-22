// The standalone package omits types for these internal widget APIs.
declare module 'monaco-editor/esm/vs/editor/browser/widget/codeEditor/codeEditorWidget.js' {
  import type { editor } from 'monaco-editor'

  export const CodeEditorWidget: new (
    container: HTMLElement,
    options: editor.IEditorConstructionOptions,
    widgetOptions: { isSimpleWidget: boolean; contributions: unknown[] },
  ) => editor.ICodeEditor
}

declare module 'monaco-editor/esm/vs/editor/browser/editorExtensions.js' {
  export const EditorExtensionsRegistry: {
    getSomeEditorContributions(ids: string[]): unknown[]
  }
}

declare module 'monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController.js' {
  export const SuggestController: { readonly ID: string }
}

declare module 'monaco-editor/esm/vs/editor/contrib/snippet/browser/snippetController2.js' {
  export const SnippetController2: { readonly ID: string }
}

declare module 'monaco-editor/esm/vs/editor/standalone/common/standaloneTheme.js' {
  import type { IDisposable } from 'monaco-editor'

  export const IStandaloneThemeService: {
    readonly type: {
      registerEditorContainer(container: HTMLElement): IDisposable
    }
  }
}

declare module 'monaco-editor/esm/vs/editor/standalone/browser/standaloneServices.js' {
  import type { IDisposable } from 'monaco-editor'
  import type { ContextKeyExpression } from 'monaco-editor/esm/vs/platform/contextkey/common/contextkey.js'

  export interface StandaloneKeybindingService {
    addDynamicKeybinding(
      command: string,
      keybinding: number,
      handler: () => void,
      when: ContextKeyExpression | undefined,
    ): IDisposable
  }

  export const StandaloneServices: {
    initialize(): {
      createInstance<T, Args extends unknown[]>(
        constructor: new (...args: Args) => T,
        ...args: Args
      ): T
    }
    get<T>(identifier: { readonly type: T }): T
  }
}

declare module 'monaco-editor/esm/vs/platform/contextkey/common/contextkey.js' {
  export interface ContextKeyExpression {
    serialize(): string
  }

  export const ContextKeyExpr: {
    has(key: string): ContextKeyExpression
    equals(key: string, value: unknown): ContextKeyExpression
    not(key: string): ContextKeyExpression
    and(...expressions: (ContextKeyExpression | undefined)[]): ContextKeyExpression | undefined
  }
}

declare module 'monaco-editor/esm/vs/platform/keybinding/common/keybinding.js' {
  export const IKeybindingService: { readonly type: unknown }
}

declare module 'monaco-editor/esm/vs/base/browser/ui/aria/aria.js' {
  export function setARIAContainer(parent: HTMLElement): void
}
