import { detectOpenVpn, updateSettings } from '../../features/settings/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { AppStoreApi } from '../createAppStore'

export function createSaveSettingsAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async (openvpnPathOverride: string | null, verboseLogging: boolean) => {
    try {
      const [settings, detection] = await Promise.all([
        updateSettings({ openvpnPathOverride, verboseLogging }),
        detectOpenVpn(),
      ])
      set({ settings, detection, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}
