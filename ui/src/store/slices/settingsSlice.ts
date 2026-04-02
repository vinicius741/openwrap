import type { HelperStatus, OpenVpnDetection, Settings } from '../../types/ipc'

export type SettingsSlice = {
  settings: Settings | null
  detection: OpenVpnDetection | null
  helperStatus: HelperStatus | null
  helperInstalling: boolean
}

export const settingsInitialState: SettingsSlice = {
  settings: null,
  detection: null,
  helperStatus: null,
  helperInstalling: false,
}
