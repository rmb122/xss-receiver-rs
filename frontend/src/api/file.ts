import request from '@/utils/request'
import type { Entry } from '@/types/file'
import { BASE_URL } from '@/config/api'

interface ListResponse {
  entries: Entry[]
}

interface ListAllResponse {
  files: string[]
}

interface PartUploadResponse {
  chunk_id: string
}

// ===== 目录/文件操作 =====

export function listDir(path: string) {
  return request.post<ListResponse>('/file/list', { path }).then((r) => r.entries)
}

export function listAll() {
  return request.post<ListAllResponse>('/file/list_all', {}).then((r) => r.files)
}

export function statFile(path: string) {
  return request.get<Entry>('/file/stat', { params: { path } })
}

export function mkdir(path: string) {
  return request.post<boolean>('/file/mkdir', { path })
}

export function remove(path: string) {
  return request.post<boolean>('/file/remove', { path })
}

export function rename(src: string, dst: string) {
  return request.post<boolean>('/file/rename', { src, dst })
}

// ===== 上传 =====

export function uploadFile(path: string, file: Blob) {
  const formData = new FormData()
  formData.append('path', path)
  formData.append('file', file)
  return request.post<boolean>('/file/upload', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
  })
}

function uploadPart(chunk: Blob, onProgress?: (progress: number) => void) {
  const formData = new FormData()
  formData.append('file', chunk)
  return request.post<PartUploadResponse>('/file/part', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
    onUploadProgress: (e) => {
      if (onProgress && e.total) {
        onProgress(e.loaded / e.total)
      }
    },
  })
}

function mergeParts(chunk_ids: string[], path: string) {
  return request.post<boolean>('/file/merge', { chunk_ids, path })
}

const CHUNK_SIZE = 1 * 1024 * 1024 // 1M

export async function chunkedUpload(
  path: string,
  file: Blob,
  onProgress?: (progress: number) => void,
) {
  const totalChunks = Math.ceil(file.size / CHUNK_SIZE)
  const chunkIds: string[] = []

  for (let i = 0; i < totalChunks; i++) {
    const start = i * CHUNK_SIZE
    const end = Math.min(start + CHUNK_SIZE, file.size)
    const chunk = file.slice(start, end)

    const response = await uploadPart(chunk, (chunkProgress) => {
      if (onProgress) {
        const overallProgress = (i + chunkProgress) / totalChunks
        onProgress(overallProgress)
      }
    })

    chunkIds.push(response.chunk_id)
  }

  await mergeParts(chunkIds, path)
  if (onProgress) onProgress(1)
}

// ===== 下载/读取 =====

export function getFileBytes(path: string): Promise<Uint8Array<ArrayBuffer>> {
  return request
    .raw<ArrayBuffer>({
      method: 'GET',
      url: `/file/download`,
      params: { path },
      responseType: 'arraybuffer',
    })
    .then((buf) => new Uint8Array(buf))
}

export function downloadFile(path: string) {
  const filename = path.split('/').pop() || 'download'
  const link = `${BASE_URL}/file/download?path=${encodeURIComponent(path)}`
  const a = document.createElement('a')
  a.href = link
  a.download = filename
  a.target = '_blank'
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
}

// ===== 日志文件（不变）=====

export function getDownloadLogFileUrl(hash: string) {
  const token = localStorage.getItem('token')
  return `${BASE_URL}/file/log/${encodeURIComponent(hash)}?authorization=${encodeURIComponent(token || '')}`
}
