// Toast 工具函数

export type ToastType = 'success' | 'error' | 'info' | 'warning'

export interface ToastMessage {
  id: string
  type: ToastType
  message: string
  duration: number
  details?: string
}

export type ToastMessageUpdateFn = (message: string, progress: number) => void

let toastHandler: ((message: ToastMessage) => ToastMessageUpdateFn) | null = null

export const registerToastHandler = (handler: (message: ToastMessage) => ToastMessageUpdateFn) => {
  toastHandler = handler
}

const createToast = (
  type: ToastType,
  message: string,
  details?: string,
  duration?: number,
): ToastMessageUpdateFn => {
  if (toastHandler) {
    return toastHandler({
      id: `${Date.now()}-${Math.random()}`,
      type,
      message,
      duration: duration === undefined ? 3000 : duration,
      details,
    })
  } else {
    console.error('Toast handler not registered:', message, details)
    return () => {}
  }
}

export const showSuccessToast = (
  message: string,
  details?: string,
  duration?: number,
): ToastMessageUpdateFn => {
  return createToast('success', message, details, duration)
}

export const showErrorToast = (
  message: string,
  details?: string,
  duration?: number,
): ToastMessageUpdateFn => {
  return createToast('error', message, details, duration)
}

export const showInfoToast = (
  message: string,
  details?: string,
  duration?: number,
): ToastMessageUpdateFn => {
  return createToast('info', message, details, duration)
}

export const showWarningToast = (
  message: string,
  details?: string,
  duration?: number,
): ToastMessageUpdateFn => {
  return createToast('warning', message, details, duration)
}
