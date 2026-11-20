import request from '@/utils/request'
import type { CreateTokenRequest, IssuedToken, TokenInfo } from '@/types/token'

export function getTokens() {
  return request.get<TokenInfo[]>('/token')
}

export function createToken(data: CreateTokenRequest) {
  return request.post<IssuedToken>('/token', data)
}

export function deleteToken(tokenId: number) {
  return request.delete<boolean>('/token', { data: { token_id: tokenId } })
}
