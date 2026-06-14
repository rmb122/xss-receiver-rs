import axios from 'axios'
import type { AxiosError, AxiosRequestConfig, AxiosResponse } from 'axios'
import { BASE_URL } from '@/config/api'
import { showErrorToast } from '@/utils/toast'
import type { ApiResponse } from '@/types/api'

export const DISABLE_ERROR_TOAST_KEY = 'DISABLE_ERROR_TOAST_KEY'
const RAW_RESPONSE_KEY = 'RAW_RESPONSE_KEY'

declare module 'axios' {
  export interface AxiosRequestConfig {
    [DISABLE_ERROR_TOAST_KEY]?: boolean
    [RAW_RESPONSE_KEY]?: boolean
  }
}

const axiosInstance = axios.create({
  baseURL: BASE_URL,
  timeout: 30000,
})

axiosInstance.interceptors.response.use(
  (response: AxiosResponse<ApiResponse<any>>) => {
    if (response.config[RAW_RESPONSE_KEY]) {
      return response
    }

    const data = response.data
    if (data.code !== 200) {
      const disableToast = response.config[DISABLE_ERROR_TOAST_KEY]
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
    const disableToast = error.config?.[DISABLE_ERROR_TOAST_KEY]
    if (!disableToast) {
      showErrorToast(error.message || '网络错误')
    }
    return Promise.reject(error)
  },
)

function unwrapPayload<T>(response: AxiosResponse<ApiResponse<T>>): T {
  return response.data.payload
}

const request = {
  get<T = any>(url: string, config?: AxiosRequestConfig): Promise<T> {
    return axiosInstance.get<ApiResponse<T>>(url, config).then(unwrapPayload<T>)
  },
  delete<T = any>(url: string, config?: AxiosRequestConfig): Promise<T> {
    return axiosInstance.delete<ApiResponse<T>>(url, config).then(unwrapPayload<T>)
  },
  post<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<T> {
    return axiosInstance.post<ApiResponse<T>>(url, data, config).then(unwrapPayload<T>)
  },
  put<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<T> {
    return axiosInstance.put<ApiResponse<T>>(url, data, config).then(unwrapPayload<T>)
  },
  patch<T = any>(url: string, data?: any, config?: AxiosRequestConfig): Promise<T> {
    return axiosInstance.patch<ApiResponse<T>>(url, data, config).then(unwrapPayload<T>)
  },
  raw<T = any>(config: AxiosRequestConfig): Promise<T> {
    return axiosInstance
      .request<T>({
        ...config,
        [RAW_RESPONSE_KEY]: true,
      })
      .then((response) => response.data)
  },
}

export default request
