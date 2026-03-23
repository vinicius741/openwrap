import { listProfiles, getProfile, getLastSelectedProfile, setLastSelectedProfile } from '../../features/profiles/api'
import { getConnectionState, getRecentLogs } from '../../features/connection/api'
import { getSettings, detectOpenVpn } from '../../features/settings/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { AppStoreApi } from '../createAppStore'

export function createLoadInitialAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async () => {
    try {
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

      set({
        profiles,
        selectedProfileId,
        selectedProfile,
        connection,
        logs: logs.map((entry, index) => ({ ...entry, id: index + 1 })),
        nextLogId: logs.length,
        settings,
        detection,
        error: null,
      })

      if (selectedProfileId && selectedProfileId !== lastSelectedProfileId) {
        await setLastSelectedProfile(selectedProfileId)
      }
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}
