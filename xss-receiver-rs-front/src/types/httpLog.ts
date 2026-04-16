export type BodyKind = 'RAW' | 'FORM' | 'JSON'

export type KeyValues = Record<string, string[]>
export type PersistedUploadFile = Record<string, [string, string][]>

export interface HttpLog {
  id: number
  client_ip: string
  client_port: number
  location: string
  method: string
  path: string
  arg: KeyValues
  header: KeyValues
  body_type: BodyKind
  body: string
  file: PersistedUploadFile
  extra_info: any
  error_log: string | null
  create_time: string
}
