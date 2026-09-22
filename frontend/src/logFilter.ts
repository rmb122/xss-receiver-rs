export type LogFilterKind = 'http' | 'dns'

interface FilterField {
  name: string
  type: 'integer' | 'text' | 'time' | 'bytes' | 'enum'
  description: string
  values?: string[]
  nullable?: boolean
}

const commonFields: FilterField[] = [
  { name: 'id', type: 'integer', description: '日志 ID' },
  { name: 'client_ip', type: 'text', description: '客户端 IP' },
  { name: 'client_port', type: 'integer', description: '客户端端口' },
  { name: 'location', type: 'text', description: '客户端位置' },
  { name: 'create_time', type: 'time', description: '创建时间, 未写偏移时使用浏览器时区' },
  { name: 'error_log', type: 'text', description: '错误信息', nullable: true },
]

const fields: Record<LogFilterKind, FilterField[]> = {
  http: [
    ...commonFields,
    { name: 'method', type: 'text', description: 'HTTP 请求方法' },
    { name: 'path', type: 'text', description: '请求路径' },
    { name: 'raw_query', type: 'text', description: '原始查询字符串' },
    { name: 'raw_body', type: 'bytes', description: '原始请求体, 按 UTF-8 字节匹配' },
    {
      name: 'parsed_body_type',
      type: 'enum',
      description: '请求体解析类型',
      values: ['NONE', 'FAILED', 'FORM', 'JSON'],
    },
  ],
  dns: [
    ...commonFields,
    { name: 'query_name', type: 'text', description: 'DNS 查询名称' },
    { name: 'query_type', type: 'text', description: 'DNS 查询类型' },
    { name: 'query_class', type: 'text', description: 'DNS 查询类别' },
  ],
}

export function getLogFilterFields(kind: LogFilterKind): readonly FilterField[] {
  return fields[kind]
}

function supportsContains(field: FilterField): boolean {
  return field.type === 'text' || field.type === 'bytes'
}

function operators(field: FilterField): string[] {
  return field.type === 'integer' || field.type === 'time'
    ? ['=', '!=', '<', '<=', '>', '>=']
    : ['=', '!=']
}

interface FilterToken {
  kind: 'word' | 'string' | 'number' | 'symbol'
  text: string
  start: number
  end: number
  closed?: boolean
}

// Keep offsets in UTF-16 code units, matching Monaco's columns.
export function tokenizeLogFilter(input: string): FilterToken[] {
  const tokens: FilterToken[] = []
  const pattern = /[A-Za-z_][A-Za-z_0-9]*|-?\d+|!=|<=|>=|&&|\|\||[^\s]/g
  let match: RegExpExecArray | null
  while ((match = pattern.exec(input))) {
    const start = match.index
    const text = match[0]
    if (text === '"' || text === "'") {
      let end = pattern.lastIndex
      let closed = false
      while (end < input.length) {
        const character = input[end++]
        if (character === '\\') {
          end = Math.min(end + 1, input.length)
        } else if (character === text) {
          closed = true
          break
        }
      }
      tokens.push({ kind: 'string', text: input.slice(start, end), start, end, closed })
      pattern.lastIndex = end
    } else {
      const kind = /^[A-Za-z_]/.test(text) ? 'word' : /^-?\d/.test(text) ? 'number' : 'symbol'
      tokens.push({ kind, text, start, end: pattern.lastIndex })
    }
  }
  return tokens
}

type CompletionState =
  | 'condition'
  | 'operator'
  | 'value'
  | 'afterCondition'
  | 'containsOpen'
  | 'containsField'
  | 'containsComma'
  | 'containsClose'

// Follow only the prefix needed for suggestions; the server validates expressions.
function completionContext(tokens: FilterToken[], kind: LogFilterKind) {
  let state: CompletionState = 'condition'
  let field: FilterField | undefined
  let contains = false
  let depth = 0
  for (const token of tokens) {
    switch (state) {
      case 'condition':
        if (token.text === '!') break
        if (token.text === '(') {
          depth++
        } else if (token.text === 'contains') {
          contains = true
          state = 'containsOpen'
        } else {
          field = fields[kind].find((candidate) => candidate.name === token.text)
          if (!field) return
          state = 'operator'
        }
        break
      case 'operator':
        if (!field || !operators(field).includes(token.text)) return
        state = 'value'
        break
      case 'value':
        if (
          !(token.kind === 'string' && token.closed) &&
          token.kind !== 'number' &&
          token.text !== 'null'
        ) {
          return
        }
        state = contains ? 'containsClose' : 'afterCondition'
        break
      case 'afterCondition':
        if (token.text === '&&' || token.text === '||') {
          field = undefined
          contains = false
          state = 'condition'
        } else if (token.text === ')' && depth > 0) {
          depth--
        } else {
          return
        }
        break
      case 'containsOpen':
        if (token.text !== '(') return
        state = 'containsField'
        break
      case 'containsField':
        field = fields[kind].find((candidate) => candidate.name === token.text)
        if (!field || !supportsContains(field)) return
        state = 'containsComma'
        break
      case 'containsComma':
        if (token.text !== ',') return
        state = 'value'
        break
      case 'containsClose':
        if (token.text !== ')') return
        contains = false
        state = 'afterCondition'
        break
    }
  }
  return { state, field, contains, depth }
}

export interface FilterSuggestion {
  label: string
  text: string
  kind: 'field' | 'function' | 'operator' | 'value'
  detail: string
  snippet?: boolean
}

const typeNames: Record<FilterField['type'], string> = {
  integer: '整数',
  text: '文本',
  time: '时间',
  bytes: '原始字节',
  enum: '枚举',
}

export function completeLogFilter(input: string, offset: number, kind: LogFilterKind) {
  const tokens = tokenizeLogFilter(input)
  let target = tokens.find((token) => {
    if (offset < token.start || offset > token.end) return false
    if (token.kind === 'string') {
      return !token.closed || offset < token.end
    }
    if (token.kind === 'symbol') {
      return (
        /^[=!<>&|]+$/.test(token.text) &&
        (offset < token.end || ['!', '<', '>', '&', '|'].includes(token.text))
      )
    }
    return true
  })
  let context = completionContext(
    tokens.filter((token) => token.end <= (target?.start ?? offset)),
    kind,
  )
  // At a condition's start, "!" is negation rather than a partial "!=".
  if (target?.text === '!' && context?.state === 'condition') {
    target = undefined
    context = completionContext(
      tokens.filter((token) => token.end <= offset),
      kind,
    )
  }
  const start = target?.start ?? offset
  const end = target?.end ?? offset
  const trailingSpace = /\s/.test(input[end] ?? '') ? '' : ' '
  const suggestions: FilterSuggestion[] = []
  if (!context) return { start, end, suggestions }
  const { state, field, contains, depth } = context

  if (target?.kind === 'string' && offset > target.start) {
    if (state === 'value' && field?.values) {
      for (const value of field.values) {
        suggestions.push({
          label: value,
          text: value + (target.closed ? '' : target.text[0]),
          kind: 'value',
          detail: field.description,
        })
      }
    }
    return {
      start: target.start + 1,
      end: target.end - (target.closed ? 1 : 0),
      suggestions,
    }
  }

  if (state === 'condition' || state === 'containsField') {
    for (const candidate of fields[kind]) {
      if (state === 'containsField' && !supportsContains(candidate)) continue
      const supported =
        operators(candidate).join(' ') + (supportsContains(candidate) ? ' contains' : '')
      suggestions.push({
        label: candidate.name,
        text: candidate.name,
        kind: 'field',
        detail: candidate.description + ' (' + typeNames[candidate.type] + '); ' + supported,
      })
    }
    if (state === 'condition') {
      const hasArguments = /^\s*\(/.test(input.slice(end))
      suggestions.push({
        label: 'contains',
        text: hasArguments
          ? 'contains'
          : kind === 'http'
            ? 'contains(${1:path}, "${2:text}")'
            : 'contains(${1:query_name}, "${2:text}")',
        kind: 'function',
        detail: 'contains(field, "text"), 按字面包含匹配',
        snippet: !hasArguments,
      })
    }
  } else if (state === 'operator' && field) {
    for (const operator of operators(field)) {
      suggestions.push({
        label: operator,
        text: operator + trailingSpace,
        kind: 'operator',
        detail: '比较',
      })
    }
  } else if (state === 'value' && field) {
    const quote = target?.kind === 'string' ? target.text[0]! : '"'
    if (field.values) {
      for (const value of field.values) {
        suggestions.push({
          label: quote + value + quote,
          text: quote + value + quote,
          kind: 'value',
          detail: field.description,
        })
      }
    } else {
      suggestions.push({
        label: typeNames[field.type],
        text:
          field.type === 'integer'
            ? '${1:0}'
            : field.type === 'time'
              ? quote + '${1:2026-09-19} ${2:12:00:00}' + quote
              : quote + '${1:text}' + quote,
        kind: 'value',
        detail:
          field.type === 'time'
            ? 'YYYY-MM-DD HH:mm:ss, 也支持带偏移的 RFC3339 时间'
            : field.description,
        snippet: true,
      })
    }
    if (field.nullable && !contains) {
      suggestions.push({ label: 'null', text: 'null', kind: 'value', detail: '没有错误信息' })
    }
  } else if (state === 'afterCondition') {
    const space = start > 0 && !/\s/.test(input[start - 1]!) ? ' ' : ''
    suggestions.push(
      {
        label: '&&',
        text: space + '&&' + trailingSpace,
        kind: 'operator',
        detail: '同时满足两个条件',
      },
      {
        label: '||',
        text: space + '||' + trailingSpace,
        kind: 'operator',
        detail: '满足任意一个条件',
      },
    )
    if (depth > 0) {
      suggestions.push({ label: ')', text: ')', kind: 'operator', detail: '结束条件分组' })
    }
  }
  return { start, end, suggestions }
}
