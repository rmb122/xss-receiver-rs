export interface ApiResponse<T> {
  code: number
  msg: string | null
  payload: T | null
}

export interface PaginatedRequest {
  page?: number
  page_size?: number
}

export interface PaginatedResponse<T> {
  data: T[]
  total: number
  page: number
  page_size: number
}
