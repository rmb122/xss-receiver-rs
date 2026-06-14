import request from '@/utils/request'
import type {
  LoginRequest,
  LoggedUser,
  UserResponse,
  CreateUserRequest,
  DeleteUserRequest,
  UpdateUserRequest,
} from '@/types/user'

export function login(data: LoginRequest) {
  return request.post<string>('/user/login', data)
}

export function getCurrentUser() {
  return request.get<LoggedUser>('/user/current')
}

export function getAllUsers() {
  return request.get<UserResponse[]>('/user')
}

export function createUser(data: CreateUserRequest) {
  return request.post<UserResponse>('/user', data)
}

export function deleteUser(data: DeleteUserRequest) {
  return request.delete<boolean>('/user', { data })
}

export function updateUser(data: UpdateUserRequest) {
  return request.patch<UserResponse>('/user', data)
}
