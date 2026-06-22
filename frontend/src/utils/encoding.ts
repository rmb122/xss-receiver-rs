import iconv from 'iconv-lite'

export const SUPPORTED_ENCODINGS = [
  'UTF-8',
  'GBK',
  'GB2312',
  'GB18030',
  'Big5',
  'Shift_JIS',
  'EUC-JP',
  'EUC-KR',
  'ISO-8859-1',
  'Windows-1252',
] as const

export type SupportedEncoding = (typeof SUPPORTED_ENCODINGS)[number]

export function decodeBytes(bytes: Uint8Array<ArrayBuffer>, encoding: string): string {
  if (bytes.length === 0) return ''
  return iconv.decode(bytes as any, encoding)
}

export function encodeBytes(text: string, encoding: string): Uint8Array<ArrayBuffer> {
  const buf = iconv.encode(text, encoding)
  return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength)
}
