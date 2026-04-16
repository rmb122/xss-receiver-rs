export interface LoginRequest {
  username: string
  password: string
}

export interface LoggedUser {
  id: number
  username: string
}

export interface UserResponse {
  id: number
  username: string
  create_time: string
}

export interface CreateUserRequest {
  username: string
  password: string
}

export interface DeleteUserRequest {
  user_id: number
}

export interface UpdateUserRequest {
  user_id: number
  username?: string
  password?: string
}
