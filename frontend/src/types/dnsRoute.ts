export type PatternKind = 'PLAIN' | 'REGEX'
export type HandlerKind = 'STATIC' | 'SCRIPT' | 'NONE'

export interface DnsRoute {
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

export interface CreateDnsRouteRequest {
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

export interface UpdateDnsRouteRequest {
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

export interface DeleteDnsRouteRequest {
  route_id: number
}
