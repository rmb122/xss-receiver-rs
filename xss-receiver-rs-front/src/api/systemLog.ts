import request from '@/utils/request'
import type { ApiResponse, PaginatedRequest, PaginatedResponse } from '@/types/api'
import type { SystemLog } from '@/types/systemLog'

export function getSystemLogs(params: PaginatedRequest) {
  return request<ApiResponse<PaginatedResponse<SystemLog>>>({
    method: 'GET',
    url: '/system_log',
    params: params,
  })
}
