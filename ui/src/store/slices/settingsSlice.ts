import type { Settings } from '../../types/ipc'

export type SettingsSlice = {
  settings: Settings | null
}

export const settingsInitialState: SettingsSlice = {
  settings: null,
}
