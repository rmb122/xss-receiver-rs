import { monaco } from './monaco'
import { setARIAContainer } from 'monaco-editor/esm/vs/base/browser/ui/aria/aria.js'
import { EditorExtensionsRegistry } from 'monaco-editor/esm/vs/editor/browser/editorExtensions.js'
import { CodeEditorWidget } from 'monaco-editor/esm/vs/editor/browser/widget/codeEditor/codeEditorWidget.js'
import { PlaceholderTextContribution } from 'monaco-editor/esm/vs/editor/contrib/placeholderText/browser/placeholderTextContribution.js'
import { SnippetController2 } from 'monaco-editor/esm/vs/editor/contrib/snippet/browser/snippetController2.js'
import { SuggestController } from 'monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController.js'
import {
  StandaloneServices,
  type StandaloneKeybindingService,
} from 'monaco-editor/esm/vs/editor/standalone/browser/standaloneServices.js'
import { IStandaloneThemeService } from 'monaco-editor/esm/vs/editor/standalone/common/standaloneTheme.js'
import { ContextKeyExpr } from 'monaco-editor/esm/vs/platform/contextkey/common/contextkey.js'
import { IKeybindingService } from 'monaco-editor/esm/vs/platform/keybinding/common/keybinding.js'

export function createLogFilterEditor(
  container: HTMLElement,
  options: monaco.editor.IEditorConstructionOptions,
  apply: () => void,
): monaco.editor.ICodeEditor {
  const instantiationService = StandaloneServices.initialize()
  const themeRegistration =
    StandaloneServices.get(IStandaloneThemeService).registerEditorContainer(container)
  if (!document.querySelector('.monaco-aria-container')) {
    setARIAContainer(document.body)
  }

  // Use the simple input context with completion and placeholder contributions.
  const editor = instantiationService.createInstance(CodeEditorWidget, container, options, {
    isSimpleWidget: true,
    contributions: EditorExtensionsRegistry.getSomeEditorContributions([
      SuggestController.ID,
      SnippetController2.ID,
      PlaceholderTextContribution.ID,
    ]),
  })
  const keybindings = StandaloneServices.get(IKeybindingService) as StandaloneKeybindingService
  const applyKeybinding = keybindings.addDynamicKeybinding(
    'apply-log-filter-' + editor.getId(),
    monaco.KeyCode.Enter,
    () => {
      if (!editor.inComposition) apply()
    },
    ContextKeyExpr.and(
      ContextKeyExpr.has('textInputFocus'),
      ContextKeyExpr.equals('editorId', editor.getId()),
      ContextKeyExpr.not('suggestWidgetVisible'),
    ),
  )
  editor.onDidDispose(() => {
    applyKeybinding.dispose()
    themeRegistration.dispose()
  })
  return editor
}
