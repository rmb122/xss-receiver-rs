import * as monaco from 'monaco-editor'
import {
  completeLogFilter,
  getLogFilterFields,
  tokenizeLogFilter,
  type FilterSuggestion,
  type LogFilterKind,
} from './logFilter'

export function logFilterLanguageId(kind: LogFilterKind): string {
  return 'log-filter-' + kind
}

const suggestionKinds: Record<FilterSuggestion['kind'], monaco.languages.CompletionItemKind> = {
  field: monaco.languages.CompletionItemKind.Field,
  function: monaco.languages.CompletionItemKind.Function,
  operator: monaco.languages.CompletionItemKind.Operator,
  value: monaco.languages.CompletionItemKind.Value,
}

// Single-line inputs do not carry tokenizer state between lines.
const tokenState: monaco.languages.IState = {
  clone() {
    return this
  },
  equals(other) {
    return other === this
  },
}

export function configureLogFilterLanguages() {
  for (const kind of ['http', 'dns'] as const) {
    const languageId = logFilterLanguageId(kind)
    const fields = new Set(getLogFilterFields(kind).map((field) => field.name))
    monaco.languages.register({ id: languageId })
    monaco.languages.setLanguageConfiguration(languageId, {
      wordPattern: /[A-Za-z_][A-Za-z_0-9]*/,
      brackets: [['(', ')']],
      autoClosingPairs: [
        { open: '(', close: ')', notIn: ['string'] },
        { open: '"', close: '"', notIn: ['string'] },
        { open: "'", close: "'", notIn: ['string'] },
      ],
    })
    monaco.languages.setTokensProvider(languageId, {
      getInitialState: () => tokenState,
      tokenize(line) {
        const tokens: monaco.languages.IToken[] = [{ startIndex: 0, scopes: '' }]
        function addToken(startIndex: number, scopes: string) {
          if (tokens[tokens.length - 1]?.startIndex === startIndex) tokens.pop()
          tokens.push({ startIndex, scopes })
        }
        for (const token of tokenizeLogFilter(line)) {
          let scope = ''
          if (token.kind === 'string' || token.kind === 'number') scope = token.kind
          else if (fields.has(token.text)) scope = 'variable'
          else if (token.text === 'contains') scope = 'type'
          else if (token.text === 'null') scope = 'keyword'
          else if (/^[=!<>&|]+$/.test(token.text)) scope = 'keyword.operator'
          else if (/^[(),]$/.test(token.text)) scope = 'delimiter'
          addToken(token.start, scope)
          if (token.kind === 'string') {
            for (const escape of token.text.matchAll(/\\(?:u[0-9a-fA-F]{4}|["'\\/bfnrt])/g)) {
              const start = token.start + escape.index
              addToken(start, 'constant.character.escape')
              if (start + escape[0].length < token.end) {
                addToken(start + escape[0].length, 'string')
              }
            }
          }
          if (token.end < line.length) addToken(token.end, '')
        }
        return { tokens, endState: tokenState }
      },
    })
    monaco.languages.registerCompletionItemProvider(languageId, {
      triggerCharacters: [' ', '(', ',', '=', '!', '<', '>', '&', '|', '"', "'"],
      provideCompletionItems(model, position, context) {
        const result = completeLogFilter(
          model.getLineContent(position.lineNumber),
          position.column - 1,
          kind,
        )
        // Finishing a quoted value should leave Enter available to apply the filter.
        if (
          (context.triggerCharacter === '"' || context.triggerCharacter === "'") &&
          result.suggestions[0]?.kind === 'operator'
        ) {
          return { suggestions: [] }
        }
        return {
          suggestions: result.suggestions.map((suggestion) => ({
            label: suggestion.label,
            insertText: suggestion.text,
            kind: suggestionKinds[suggestion.kind],
            detail: suggestion.detail,
            insertTextRules: suggestion.snippet
              ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
              : undefined,
            range: new monaco.Range(
              position.lineNumber,
              result.start + 1,
              position.lineNumber,
              result.end + 1,
            ),
          })),
        }
      },
    })
  }
}
