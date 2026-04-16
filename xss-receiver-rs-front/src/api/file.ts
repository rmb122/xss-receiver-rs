import request from '@/utils/request'
import type { ApiResponse } from '@/types/api'
import type { FileList, RenameRequest, MergeRequest, PartUploadResponse } from '@/types/file'
import { BASE_URL } from '@/config/api'
import type { AxiosRequestConfig } from 'axios'

// 目录操作
export function listAllFiles() {
  return request.get<ApiResponse<FileList>>('/file/user/')
}

export function createDirectory(directory: string) {
  return request.post<ApiResponse<boolean>>(`/file/user/${encodeURIComponent(directory)}/`)
}

export function deleteDirectory(directory: string) {
  return request.delete<ApiResponse<boolean>>(`/file/user/${encodeURIComponent(directory)}/`)
}

export function renameDirectory(directory: string, data: RenameRequest) {
  return request.patch<ApiResponse<boolean>>(`/file/user/${encodeURIComponent(directory)}/`, data)
}

// 文件操作
export function listDirectoryFiles(directory: string) {
  return request.get<ApiResponse<string[]>>(`/file/user/${encodeURIComponent(directory)}/`)
}

export function uploadFile(directory: string, filename: string, file: File | Blob) {
  const formData = new FormData()
  formData.append('file', file, filename)
  return request.post<ApiResponse<boolean>>(
    `/file/user/${encodeURIComponent(directory)}/${encodeURIComponent(filename)}`,
    formData,
    { headers: { 'Content-Type': 'multipart/form-data' } },
  )
}

export function downloadFile(directory: string, filename: string) {
  const link = `${BASE_URL}/file/user/${encodeURIComponent(directory)}/${encodeURIComponent(filename)}`
  const a = document.createElement('a')
  a.href = link
  a.download = filename
  a.target = '_blank'
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
}

export function getFileContent(directory: string, filename: string) {
  return request.get(
    `/file/user/${encodeURIComponent(directory)}/${encodeURIComponent(filename)}`,
    {
      responseType: 'text',
      transformResponse: [(data: any) => data],
    },
  )
}

export function deleteFile(directory: string, filename: string) {
  return request.delete<ApiResponse<boolean>>(
    `/file/user/${encodeURIComponent(directory)}/${encodeURIComponent(filename)}`,
  )
}

export function renameFile(directory: string, filename: string, data: RenameRequest) {
  return request.patch<ApiResponse<boolean>>(
    `/file/user/${encodeURIComponent(directory)}/${encodeURIComponent(filename)}`,
    data,
  )
}

// 分片上传
export function uploadPart(chunk: Blob, onProgress?: (progress: number) => void) {
  const formData = new FormData()
  formData.append('file', chunk)
  return request.post<ApiResponse<PartUploadResponse>>('/file/temp/part', formData, {
    headers: { 'Content-Type': 'multipart/form-data' },
    onUploadProgress: (e) => {
      if (onProgress && e.total) {
        onProgress(e.loaded / e.total)
      }
    },
  })
}

export function mergeParts(data: MergeRequest) {
  return request.post<ApiResponse<boolean>>('/file/temp/merge', data)
}

// 分片上传完整流程
const CHUNK_SIZE = 0.9 * 1024 * 1024 // 900k

export async function chunkedUpload(
  directory: string,
  filename: string,
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

    chunkIds.push(response.data.payload!.chunk_id)
  }

  await mergeParts({ chunk_ids: chunkIds, directory, filename })
  if (onProgress) onProgress(1)
}

// 日志文件下载
export function downloadLogFile(hash: string) {
  const token = localStorage.getItem('token')
  // 直接构建 URL 用于下载
  return `${BASE_URL}/file/log/${encodeURIComponent(hash)}?authorization=${encodeURIComponent(token || '')}`
}

export function downloadLogFileBlob(hash: string) {
  return request.get(`/file/log/${encodeURIComponent(hash)}`, {
    responseType: 'blob',
  })
}
