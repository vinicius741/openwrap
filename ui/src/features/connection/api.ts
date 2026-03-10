import { invokeCommand } from '../../lib/tauri'
import type { ConnectionSnapshot, LogEntry } from '../../types/ipc'

export async function connectProfile(profileId: string) {
  return invokeCommand<ConnectionSnapshot>('connect', { profileId })
}

export async function disconnectProfile() {
  return invokeCommand<ConnectionSnapshot>('disconnect')
}

export async function getConnectionState() {
  return invokeCommand<ConnectionSnapshot>('get_connection_state')
}

export async function getRecentLogs(limit = 200) {
  return invokeCommand<LogEntry[]>('get_recent_logs', { limit })
}

export async function submitCredentials(input: {
  profileId: string
  username: string
  password: string
  rememberInKeychain: boolean
}) {
  return invokeCommand<ConnectionSnapshot>('submit_credentials', {
    request: input,
  })
}
