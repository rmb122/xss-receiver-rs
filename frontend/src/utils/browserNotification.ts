const DEFAULT_NOTIFICATION_TITLE = 'XSS Receiver'
const DEFAULT_NOTIFICATION_ICON = '/favicon.ico'

interface BrowserNotificationOptions {
  title?: string
  body: string
  tag?: string
  icon?: string
}

export function isBrowserNotificationSupported(): boolean {
  return typeof window !== 'undefined' && 'Notification' in window
}

export async function requestBrowserNotificationPermission(): Promise<NotificationPermission | null> {
  if (!isBrowserNotificationSupported()) {
    return null
  }

  if (Notification.permission === 'default') {
    return Notification.requestPermission()
  }

  return Notification.permission
}

export function sendBrowserNotification(options: BrowserNotificationOptions): boolean {
  if (!isBrowserNotificationSupported() || Notification.permission !== 'granted') {
    return false
  }

  new Notification(options.title ?? DEFAULT_NOTIFICATION_TITLE, {
    body: options.body,
    icon: options.icon ?? DEFAULT_NOTIFICATION_ICON,
    tag: options.tag,
  })

  return true
}
