import request from '@/utils/request'
import type {
  HttpRoute,
  CreateHttpRouteRequest,
  UpdateHttpRouteRequest,
  DeleteHttpRouteRequest,
} from '@/types/httpRoute'

export function getAllHttpRoutes() {
  return request.get<HttpRoute[]>('/http_route')
}

export function createHttpRoute(data: CreateHttpRouteRequest) {
  return request.post<HttpRoute>('/http_route', data)
}

export function deleteHttpRoute(data: DeleteHttpRouteRequest) {
  return request.delete<boolean>('/http_route', { data })
}

export function updateHttpRoute(data: UpdateHttpRouteRequest) {
  return request.patch<HttpRoute>('/http_route', data)
}
