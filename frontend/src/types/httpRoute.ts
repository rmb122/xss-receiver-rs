export type PatternKind = 'PLAIN' | 'REGEX'
export type HandlerKind = 'STATIC' | 'SCRIPT' | 'NONE'

export interface HttpRoute {
  id: number
  pattern_kind: PatternKind
  pattern: string
  priority: number
  timeout: number
  catalog: string
  handler_kind: HandlerKind
  handler: string
  write_log: boolean
  comment: string
  create_time: string
}

export interface CreateHttpRouteRequest {
  pattern_kind: PatternKind
  pattern: string
  priority: number
  timeout: number
  catalog: string
  handler_kind: HandlerKind
  handler: string
  write_log: boolean
  comment: string
}

export interface UpdateHttpRouteRequest {
  http_route_id: number
  pattern_kind: PatternKind
  pattern: string
  priority: number
  timeout: number
  catalog: string
  handler_kind: HandlerKind
  handler: string
  write_log: boolean
  comment: string
}

export interface DeleteHttpRouteRequest {
  http_route_id: number
}
