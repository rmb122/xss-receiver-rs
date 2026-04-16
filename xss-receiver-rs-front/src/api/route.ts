import request from '@/utils/request'
import type { ApiResponse } from '@/types/api'
import type {
  Route,
  CreateRouteRequest,
  UpdateRouteRequest,
  DeleteRouteRequest,
} from '@/types/route'

export function getAllRoutes() {
  return request.get<ApiResponse<Route[]>>('/route')
}

export function createRoute(data: CreateRouteRequest) {
  return request.post<ApiResponse<Route>>('/route', data)
}

export function deleteRoute(data: DeleteRouteRequest) {
  return request.delete<ApiResponse<boolean>>('/route', { data })
}

export function updateRoute(data: UpdateRouteRequest) {
  return request.patch<ApiResponse<Route>>('/route', data)
}
