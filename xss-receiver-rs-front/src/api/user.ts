import request from '@/utils/request'
import type { ApiResponse } from '@/types/api'
import type {
  LoginRequest,
  LoggedUser,
  UserResponse,
  CreateUserRequest,
  DeleteUserRequest,
  UpdateUserRequest,
} from '@/types/user'

export function login(data: LoginRequest) {
  return request.post<ApiResponse<string>>('/user/login', data)
}

export function getCurrentUser() {
  return request.get<ApiResponse<LoggedUser>>('/user/current')
}

export function getAllUsers() {
  return request.get<ApiResponse<UserResponse[]>>('/user')
}

export function createUser(data: CreateUserRequest) {
  return request.post<ApiResponse<UserResponse>>('/user', data)
}

export function deleteUser(data: DeleteUserRequest) {
  return request.delete<ApiResponse<boolean>>('/user', { data })
}

export function updateUser(data: UpdateUserRequest) {
  return request.patch<ApiResponse<UserResponse>>('/user', data)
}
