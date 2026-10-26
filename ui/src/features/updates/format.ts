import type { UpdateProgress } from '../../types/ipc'

export function formatUpdateProgress(progress: UpdateProgress | null) {
  if (!progress) return 'Starting download…'
  const mb = (bytes: number) => (bytes / 1024 / 1024).toFixed(1)
  if (progress.total) {
    const percent = Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
    return `Downloading… ${percent}% (${mb(progress.downloaded)} / ${mb(progress.total)} MB)`
  }
  return `Downloading… ${mb(progress.downloaded)} MB`
}
