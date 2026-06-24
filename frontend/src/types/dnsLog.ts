export interface DnsLog {
  id: number
  client_ip: string
  client_port: number
  location: string
  query_name: string
  query_type: string
  query_class: string
  extra_info: any
  error_log: string | null
  create_time: string
}
