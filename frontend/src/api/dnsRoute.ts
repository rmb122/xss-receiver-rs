import request from '@/utils/request'
import type {
  DnsRoute,
  CreateDnsRouteRequest,
  UpdateDnsRouteRequest,
  DeleteDnsRouteRequest,
} from '@/types/dnsRoute'

export function getAllDnsRoutes() {
  return request.get<DnsRoute[]>('/dns_route')
}

export function createDnsRoute(data: CreateDnsRouteRequest) {
  return request.post<DnsRoute>('/dns_route', data)
}

export function deleteDnsRoute(data: DeleteDnsRouteRequest) {
  return request.delete<boolean>('/dns_route', { data })
}

export function updateDnsRoute(data: UpdateDnsRouteRequest) {
  return request.patch<DnsRoute>('/dns_route', data)
}
