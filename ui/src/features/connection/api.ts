import { invoke } from '@tauri-apps/api/core'

import type { ConnectionSnapshot, LogEntry } from '../../types/ipc'

export async function connectProfile(profileId: string) {
  return invoke<ConnectionSnapshot>('connect', { profileId })
}

export async function disconnectProfile() {
  return invoke<ConnectionSnapshot>('disconnect')
}

export async function getConnectionState() {
  return invoke<ConnectionSnapshot>('get_connection_state')
}

export async function getRecentLogs(limit = 200) {
  return invoke<LogEntry[]>('get_recent_logs', { limit })
}

export async function submitCredentials(input: {
  profileId: string
  username: string
  password: string
  rememberInKeychain: boolean
}) {
  return invoke<ConnectionSnapshot>('submit_credentials', {
    request: input,
  })
}

