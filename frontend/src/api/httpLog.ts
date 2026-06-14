import request from '@/utils/request'
import type { PaginatedRequest, PaginatedResponse } from '@/types/api'
import type { HttpLog } from '@/types/httpLog'

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
