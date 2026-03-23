import type { OpenVpnDetection, Settings } from '../../types/ipc'

export type SettingsSlice = {
  settings: Settings | null
  detection: OpenVpnDetection | null
}

export const settingsInitialState: SettingsSlice = {
  settings: null,
  detection: null,
}
