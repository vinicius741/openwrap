import { invokeCommand } from '../../lib/tauri'
import { getRecentLogs } from '../connection/api'
import type { SessionSummary } from '../../types/ipc'

export { getRecentLogs }

export async function revealConnectionLogInFinder() {
  return invokeCommand<void>('reveal_connection_log_in_finder')
}

export async function revealLogsFolder() {
  return invokeCommand<void>('reveal_logs_folder')
}

export async function getRecentSessions(limit = 10) {
  return invokeCommand<SessionSummary[]>('get_recent_sessions', { limit })
}

export async function cleanupOldLogs(maxAgeDays = 30) {
  return invokeCommand<number>('cleanup_old_logs', { maxAgeDays })
}
