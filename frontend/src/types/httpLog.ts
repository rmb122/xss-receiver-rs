export type BodyKind = 'NONE' | 'FAILED' | 'FORM' | 'JSON'

export type KeyValues = Record<string, string[]>
export type PersistedUploadFile = Record<string, [string, string][]>

export interface HttpLog {
  id: number
  client_ip: string
  client_port: number
  location: string
  method: string
  path: string
  raw_query: string
  parsed_query: KeyValues
  header: KeyValues
  parsed_body_type: BodyKind
  parsed_body: string
  file: PersistedUploadFile
  extra_info: any
  error_log: string | null
  create_time: string
}
