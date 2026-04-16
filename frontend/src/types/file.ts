export interface RenameRequest {
  new_directory?: string
  new_name: string
}

export interface MergeRequest {
  chunk_ids: string[]
  directory: string
  filename: string
}

export interface PartUploadResponse {
  chunk_id: string
}

export interface FileInfo {
  /** 文件名 */
  name: string
  /** 文件大小（字节） */
  size: number
  /** 最后修改时间（Unix 时间戳，秒） */
  modified_time: number
}

export type FileList = Record<string, FileInfo[]>

export interface FileTableItem extends FileInfo {
  /** 所属目录 */
  directory: string
  /** 完整路径（用于操作） */
  path: string
  /** 标记是否为空目录占位项 */
  isEmpty?: boolean
}
