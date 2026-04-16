export type PatternKind = 'PLAIN' | 'REGEX'
export type HandlerKind = 'STATIC' | 'SCRIPT' | 'NONE'

export interface Route {
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

export interface CreateRouteRequest {
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

export interface UpdateRouteRequest {
  route_id: number
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

export interface DeleteRouteRequest {
  route_id: number
}
