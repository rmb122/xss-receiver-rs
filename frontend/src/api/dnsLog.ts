import request, { DISABLE_ERROR_TOAST_KEY } from '@/utils/request'
import type { LogQuery, PaginatedResponse } from '@/types/api'
import type { DnsLog } from '@/types/dnsLog'

export function getDnsLogs(params: LogQuery) {
  return request.get<PaginatedResponse<DnsLog>>('/dns_log', {
    params,
    [DISABLE_ERROR_TOAST_KEY]: true,
  })
}
