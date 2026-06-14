import request from '@/utils/request'
import type {
  Route,
  CreateRouteRequest,
  UpdateRouteRequest,
  DeleteRouteRequest,
} from '@/types/route'

export function getAllRoutes() {
  return request.get<Route[]>('/route')
}

export function createRoute(data: CreateRouteRequest) {
  return request.post<Route>('/route', data)
}

export function deleteRoute(data: DeleteRouteRequest) {
  return request.delete<boolean>('/route', { data })
}

export function updateRoute(data: UpdateRouteRequest) {
  return request.patch<Route>('/route', data)
}
