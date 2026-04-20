export type EntryKind = 'file' | 'directory'

export interface Entry {
  name: string
  kind: EntryKind
  size: number
  modified_time: number
}
