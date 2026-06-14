import request from '@/utils/request'
import type { ApiResponse, PaginatedRequest, PaginatedResponse } from '@/types/api'
import type { HttpLog } from '@/types/httpLog'

export function getHttpLogs(params: PaginatedRequest) {
  return request<ApiResponse<PaginatedResponse<HttpLog>>>({
    method: 'GET',
    url: '/http_log',
    params: params,
  })
}

export function getHttpLogRawBody(id: number) {
  return request<ArrayBuffer>({
    method: 'GET',
    url: `/http_log/${id}/raw_body`,
    responseType: 'arraybuffer',
  })
}
