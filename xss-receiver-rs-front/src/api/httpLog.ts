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
