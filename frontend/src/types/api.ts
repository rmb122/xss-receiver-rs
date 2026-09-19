export interface ApiResponse<T> {
  code: number
  msg: string | null
  payload: T
}

export interface PaginatedRequest {
  page?: number
  page_size?: number
}

export interface LogQuery extends PaginatedRequest {
  filter?: string
  timezone?: string
}

export interface PaginatedResponse<T> {
  data: T[]
  total: number
  page: number
  page_size: number
}
