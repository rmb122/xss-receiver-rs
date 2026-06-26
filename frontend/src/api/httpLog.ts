import request from '@/utils/request'
import type { PaginatedRequest, PaginatedResponse } from '@/types/api'
import type { HttpLog } from '@/types/httpLog'
import { BASE_URL } from '@/config/api'

export function getHttpLogs(params: PaginatedRequest) {
  return request.get<PaginatedResponse<HttpLog>>('/http_log', {
    params,
  })
}

export function getHttpLogRawBody(id: number) {
  return request.raw<ArrayBuffer>({
    method: 'GET',
    url: `/http_log/${id}/raw_body`,
    responseType: 'arraybuffer',
  })
}

export function getHttpLogRawBodyUrl(id: number) {
  return `${BASE_URL}/http_log/${id}/raw_body`
}

export function downloadHttpLogRawBody(id: number) {
  // 后端 raw_body 接口不返回 Content-Disposition, 这里指定包含 http_log id 的文件名
  const filename = `http-log-${id}.body`
  const link = getHttpLogRawBodyUrl(id)
  const a = document.createElement('a')
  a.href = link
  a.download = filename
  a.target = '_blank'
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
}
