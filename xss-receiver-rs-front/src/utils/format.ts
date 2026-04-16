export function formatTime(time: string | number): string {
  if (!time && time !== 0) return '-'
  try {
    const d = typeof time === 'number' ? new Date(time * 1000) : new Date(time)
    return d.toLocaleString('zh-CN', { hour12: false })
  } catch {
    return typeof time === 'string' ? time : '-'
  }
}

export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`
}
