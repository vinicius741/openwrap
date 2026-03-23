import { listProfiles, getProfile, getLastSelectedProfile, setLastSelectedProfile } from '../../features/profiles/api'
import { getConnectionState, getRecentLogs } from '../../features/connection/api'
import { getSettings, detectOpenVpn } from '../../features/settings/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { ProfileSummary, ProfileDetail, ConnectionSnapshot, LogEntry, Settings, OpenVpnDetection, UserFacingError } from '../../types/ipc'

export type LoadInitialResult = {
  profiles: ProfileSummary[]
  selectedProfileId: string | null
  selectedProfile: ProfileDetail | null
  connection: ConnectionSnapshot | null
  logs: Array<LogEntry & { id: number }>
  nextLogId: number
  settings: Settings | null
  detection: OpenVpnDetection | null
  error: UserFacingError | null
}

export async function loadInitial(): Promise<LoadInitialResult> {
  const [profiles, connection, logs, settings, detection, lastSelectedProfileId] = await Promise.all([
    listProfiles(),
    getConnectionState(),
    getRecentLogs(),
    getSettings(),
    detectOpenVpn(),
    getLastSelectedProfile(),
  ])

  const selectedProfileId =
    lastSelectedProfileId && profiles.some((profile) => profile.id === lastSelectedProfileId)
      ? lastSelectedProfileId
      : profiles[0]?.id ?? null
  const selectedProfile = selectedProfileId ? await getProfile(selectedProfileId) : null

  const result = {
    profiles,
    selectedProfileId,
    selectedProfile,
    connection,
    logs: logs.map((entry, index) => ({ ...entry, id: index + 1 })),
    nextLogId: logs.length,
    settings,
    detection,
    error: null as UserFacingError | null,
  }

  if (selectedProfileId && selectedProfileId !== lastSelectedProfileId) {
    await setLastSelectedProfile(selectedProfileId)
  }

  return result
}

export function loadInitialError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}
