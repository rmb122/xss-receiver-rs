// 必须在导入 monaco-editor 之前加载中文语言包：
// 它会设置 globalThis._VSCODE_NLS_MESSAGES，供 monaco 懒加载本地化字符串时读取
import 'monaco-editor/esm/nls.messages.zh-cn.js'
import * as monaco from 'monaco-editor'
import { jsonDefaults } from 'monaco-editor/esm/vs/language/json/monaco.contribution'

// @ts-ignore
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
// @ts-ignore
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'
// @ts-ignore
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker'
// @ts-ignore
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker'
// @ts-ignore
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'

self.MonacoEnvironment = {
  getWorker(_: any, label: string) {
    if (label === 'json') return new jsonWorker()
    if (label === 'css' || label === 'scss' || label === 'less') return new cssWorker()
    if (label === 'html' || label === 'handlebars' || label === 'razor') return new htmlWorker()
    if (label === 'typescript' || label === 'javascript') return new tsWorker()
    return new editorWorker()
  },
}

// 注册 handler 脚本扩展名为 JavaScript 语言，使 Monaco 能自动识别脚本文件
monaco.languages.register({
  id: 'javascript',
  extensions: ['.hjs', '.djs'],
})

monaco.languages.register({
  id: 'json',
  extensions: ['.djson'],
})

jsonDefaults.setDiagnosticsOptions({
  validate: true,
  schemas: [
    {
      uri: 'schema://xss-receiver/dns-response.djson',
      fileMatch: ['*.djson', '**/*.djson'],
      schema: {
        type: 'object',
        additionalProperties: false,
        properties: {
          rcode: {
            type: 'string',
            enum: ['NOERROR', 'NXDOMAIN', 'SERVFAIL', 'REFUSED', 'FORMERR', 'NOTIMP'],
            default: 'NOERROR',
          },
          ttl: {
            type: 'integer',
            minimum: 0,
            default: 60,
          },
          answers: {
            type: 'array',
            items: {
              type: 'object',
              additionalProperties: false,
              required: ['type', 'value'],
              properties: {
                type: {
                  type: 'string',
                  enum: ['A', 'AAAA', 'CNAME', 'TXT'],
                },
                value: {
                  type: 'string',
                },
                ttl: {
                  type: 'integer',
                  minimum: 0,
                },
              },
            },
            default: [],
          },
        },
      },
    },
  ],
})

export { monaco }
