// 必须在导入 monaco-editor 之前加载中文语言包：
// 它会设置 globalThis._VSCODE_NLS_MESSAGES，供 monaco 懒加载本地化字符串时读取
import 'monaco-editor/esm/nls.messages.zh-cn.js'
import * as monaco from 'monaco-editor'

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

// 注册 .xjs 扩展名为 JavaScript 语言，使 Monaco 能自动识别 .xjs 文件
monaco.languages.register({
  id: 'javascript',
  extensions: ['.xjs'],
})

export { monaco }
