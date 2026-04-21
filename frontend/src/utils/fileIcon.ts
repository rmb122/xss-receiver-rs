/**
 * 根据文件名/扩展名返回合适的 Material Design Icon 名称和颜色
 */

interface FileIcon {
  icon: string
  color: string
}

const DEFAULT_ICON: FileIcon = { icon: 'mdi-file-outline', color: 'grey-darken-1' }

// 精确文件名映射（优先于扩展名匹配）
const FILENAME_MAP: Record<string, FileIcon> = {
  'dockerfile': { icon: 'mdi-docker', color: 'blue' },
  '.dockerignore': { icon: 'mdi-docker', color: 'blue' },
  '.gitignore': { icon: 'mdi-git', color: 'orange-darken-2' },
  '.gitattributes': { icon: 'mdi-git', color: 'orange-darken-2' },
  'package.json': { icon: 'mdi-nodejs', color: 'green-darken-2' },
  'pnpm-lock.yaml': { icon: 'mdi-nodejs', color: 'orange-darken-2' },
  'yarn.lock': { icon: 'mdi-nodejs', color: 'blue' },
  'package-lock.json': { icon: 'mdi-nodejs', color: 'red' },
  'tsconfig.json': { icon: 'mdi-language-typescript', color: 'blue' },
  'cargo.toml': { icon: 'mdi-language-rust', color: 'orange-darken-4' },
  'cargo.lock': { icon: 'mdi-language-rust', color: 'orange-darken-4' },
  'readme.md': { icon: 'mdi-information-outline', color: 'blue' },
  'license': { icon: 'mdi-license', color: 'amber-darken-2' },
  'makefile': { icon: 'mdi-cog-outline', color: 'grey-darken-2' },
}

// 按扩展名映射
const EXT_MAP: Record<string, FileIcon> = {
  // JavaScript / TypeScript
  js: { icon: 'mdi-language-javascript', color: 'amber-darken-2' },
  mjs: { icon: 'mdi-language-javascript', color: 'amber-darken-2' },
  cjs: { icon: 'mdi-language-javascript', color: 'amber-darken-2' },
  jsx: { icon: 'mdi-language-javascript', color: 'amber-darken-2' },
  xjs: { icon: 'mdi-language-javascript', color: 'deep-purple' }, // 项目自定义脚本
  ts: { icon: 'mdi-language-typescript', color: 'blue' },
  tsx: { icon: 'mdi-language-typescript', color: 'blue' },

  // Web
  html: { icon: 'mdi-language-html5', color: 'orange-darken-2' },
  htm: { icon: 'mdi-language-html5', color: 'orange-darken-2' },
  css: { icon: 'mdi-language-css3', color: 'blue' },
  scss: { icon: 'mdi-sass', color: 'pink' },
  sass: { icon: 'mdi-sass', color: 'pink' },
  less: { icon: 'mdi-language-css3', color: 'indigo' },
  vue: { icon: 'mdi-vuejs', color: 'green' },

  // Data / config
  json: { icon: 'mdi-code-json', color: 'amber-darken-2' },
  yaml: { icon: 'mdi-cog-outline', color: 'red' },
  yml: { icon: 'mdi-cog-outline', color: 'red' },
  toml: { icon: 'mdi-cog-outline', color: 'orange-darken-2' },
  xml: { icon: 'mdi-xml', color: 'orange-darken-2' },
  ini: { icon: 'mdi-cog-outline', color: 'grey-darken-1' },
  env: { icon: 'mdi-cog-outline', color: 'yellow-darken-3' },
  conf: { icon: 'mdi-cog-outline', color: 'grey-darken-1' },

  // Markdown / docs
  md: { icon: 'mdi-language-markdown', color: 'blue-grey-darken-2' },
  markdown: { icon: 'mdi-language-markdown', color: 'blue-grey-darken-2' },
  txt: { icon: 'mdi-file-document-outline', color: 'grey-darken-1' },
  rtf: { icon: 'mdi-file-document-outline', color: 'blue' },
  pdf: { icon: 'mdi-file-pdf-box', color: 'red' },
  doc: { icon: 'mdi-file-word-outline', color: 'blue-darken-2' },
  docx: { icon: 'mdi-file-word-outline', color: 'blue-darken-2' },
  xls: { icon: 'mdi-file-excel-outline', color: 'green-darken-2' },
  xlsx: { icon: 'mdi-file-excel-outline', color: 'green-darken-2' },
  ppt: { icon: 'mdi-file-powerpoint-outline', color: 'red-darken-2' },
  pptx: { icon: 'mdi-file-powerpoint-outline', color: 'red-darken-2' },

  // Code (other)
  rs: { icon: 'mdi-language-rust', color: 'orange-darken-4' },
  py: { icon: 'mdi-language-python', color: 'blue' },
  java: { icon: 'mdi-language-java', color: 'red' },
  kt: { icon: 'mdi-language-kotlin', color: 'purple' },
  go: { icon: 'mdi-language-go', color: 'cyan' },
  c: { icon: 'mdi-language-c', color: 'blue' },
  h: { icon: 'mdi-language-c', color: 'blue-darken-2' },
  cpp: { icon: 'mdi-language-cpp', color: 'blue' },
  hpp: { icon: 'mdi-language-cpp', color: 'blue-darken-2' },
  cs: { icon: 'mdi-language-csharp', color: 'purple' },
  php: { icon: 'mdi-language-php', color: 'indigo' },
  rb: { icon: 'mdi-language-ruby', color: 'red' },
  swift: { icon: 'mdi-language-swift', color: 'orange' },
  lua: { icon: 'mdi-language-lua', color: 'blue' },
  r: { icon: 'mdi-language-r', color: 'blue' },
  sql: { icon: 'mdi-database', color: 'blue-grey-darken-2' },

  // Shell
  sh: { icon: 'mdi-console', color: 'grey-darken-2' },
  bash: { icon: 'mdi-console', color: 'grey-darken-2' },
  zsh: { icon: 'mdi-console', color: 'grey-darken-2' },
  fish: { icon: 'mdi-console', color: 'grey-darken-2' },
  ps1: { icon: 'mdi-console', color: 'blue-darken-2' },
  bat: { icon: 'mdi-console', color: 'grey-darken-2' },
  cmd: { icon: 'mdi-console', color: 'grey-darken-2' },

  // Images
  png: { icon: 'mdi-file-image-outline', color: 'green' },
  jpg: { icon: 'mdi-file-image-outline', color: 'green' },
  jpeg: { icon: 'mdi-file-image-outline', color: 'green' },
  gif: { icon: 'mdi-file-image-outline', color: 'green' },
  bmp: { icon: 'mdi-file-image-outline', color: 'green' },
  webp: { icon: 'mdi-file-image-outline', color: 'green' },
  svg: { icon: 'mdi-svg', color: 'orange-darken-2' },
  ico: { icon: 'mdi-file-image-outline', color: 'blue' },

  // Audio
  mp3: { icon: 'mdi-file-music-outline', color: 'purple' },
  wav: { icon: 'mdi-file-music-outline', color: 'purple' },
  ogg: { icon: 'mdi-file-music-outline', color: 'purple' },
  flac: { icon: 'mdi-file-music-outline', color: 'purple' },
  m4a: { icon: 'mdi-file-music-outline', color: 'purple' },

  // Video
  mp4: { icon: 'mdi-file-video-outline', color: 'red' },
  mkv: { icon: 'mdi-file-video-outline', color: 'red' },
  avi: { icon: 'mdi-file-video-outline', color: 'red' },
  mov: { icon: 'mdi-file-video-outline', color: 'red' },
  webm: { icon: 'mdi-file-video-outline', color: 'red' },

  // Archives
  zip: { icon: 'mdi-zip-box-outline', color: 'amber-darken-2' },
  tar: { icon: 'mdi-zip-box-outline', color: 'amber-darken-2' },
  gz: { icon: 'mdi-zip-box-outline', color: 'amber-darken-2' },
  bz2: { icon: 'mdi-zip-box-outline', color: 'amber-darken-2' },
  xz: { icon: 'mdi-zip-box-outline', color: 'amber-darken-2' },
  '7z': { icon: 'mdi-zip-box-outline', color: 'amber-darken-2' },
  rar: { icon: 'mdi-zip-box-outline', color: 'amber-darken-2' },

  // Fonts
  ttf: { icon: 'mdi-format-font', color: 'blue-grey' },
  otf: { icon: 'mdi-format-font', color: 'blue-grey' },
  woff: { icon: 'mdi-format-font', color: 'blue-grey' },
  woff2: { icon: 'mdi-format-font', color: 'blue-grey' },

  // Keys / certs
  pem: { icon: 'mdi-key-outline', color: 'amber-darken-2' },
  key: { icon: 'mdi-key-outline', color: 'amber-darken-2' },
  crt: { icon: 'mdi-certificate-outline', color: 'green-darken-2' },
  cer: { icon: 'mdi-certificate-outline', color: 'green-darken-2' },
}

/**
 * 文件夹图标：根据展开状态选择
 */
export function folderIcon(expanded: boolean): FileIcon {
  return {
    icon: expanded ? 'mdi-folder-open' : 'mdi-folder',
    color: 'amber-darken-2',
  }
}

/**
 * 文件图标：根据文件名/扩展名匹配
 */
export function fileIcon(filename: string): FileIcon {
  const lower = filename.toLowerCase()
  if (FILENAME_MAP[lower]) return FILENAME_MAP[lower]

  const dotIdx = lower.lastIndexOf('.')
  if (dotIdx > 0 && dotIdx < lower.length - 1) {
    const ext = lower.substring(dotIdx + 1)
    if (EXT_MAP[ext]) return EXT_MAP[ext]
  }
  return DEFAULT_ICON
}
