export interface TokenInfo {
  id: number
  user_id: number
  name: string
  username: string
  create_time: string
  expires_at: string | null
  allowed_ip: string | null
}

export interface CreateTokenRequest {
  name: string
  expires_at: string | null
  allowed_ip: string | null
}

export interface IssuedToken extends TokenInfo {
  token: string
}
