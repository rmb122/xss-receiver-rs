import axios from 'axios'
import type { AxiosError, AxiosInstance, AxiosResponse } from 'axios'
import { BASE_URL } from '@/config/api'
import { showErrorToast } from '@/utils/toast'
import type { ApiResponse } from '@/types/api'

export const DISABLE_ERROR_TOAST_KEY = 'DISABLE_ERROR_TOAST_KEY'

const request: AxiosInstance = axios.create({
  baseURL: BASE_URL,
  timeout: 30000,
})

request.interceptors.response.use(
  (response: AxiosResponse<ApiResponse<any>>) => {
    const data = response.data
    if (data.code !== undefined && data.code !== 200) {
      const disableToast = (response.config as any)[DISABLE_ERROR_TOAST_KEY]
      if (!disableToast) {
        showErrorToast(data.msg || '请求失败')
      }
      if (data.code === 400) {
        // Cookie 失效，跳转登录
        if (window.location.hash !== '#/login') {
          window.location.hash = '#/login'
        }
      }
      return Promise.reject(new Error(data.msg || '请求失败'))
    }
    return response
  },
  (error: AxiosError) => {
    const disableToast = (error.config as any)[DISABLE_ERROR_TOAST_KEY]
    if (!disableToast) {
      showErrorToast(error.message || '网络错误')
    }
    return Promise.reject(error)
  },
)

export default request
