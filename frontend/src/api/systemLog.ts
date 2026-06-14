import request from '@/utils/request'
import type { PaginatedRequest, PaginatedResponse } from '@/types/api'
import type { SystemLog } from '@/types/systemLog'

export function getSystemLogs(params: PaginatedRequest) {
  return request.get<PaginatedResponse<SystemLog>>('/system_log', {
    params,
  })
}
