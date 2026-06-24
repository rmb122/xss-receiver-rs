import request from '@/utils/request'
import type { PaginatedRequest, PaginatedResponse } from '@/types/api'
import type { DnsLog } from '@/types/dnsLog'

export function getDnsLogs(params: PaginatedRequest) {
  return request.get<PaginatedResponse<DnsLog>>('/dns_log', {
    params,
  })
}
