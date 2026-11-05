import type { RuntimeDependencies, Settings, UserFacingError } from '../../types/ipc'

export type SettingsSlice = {
  settings: Settings | null
  runtimeDependencies: RuntimeDependencies | null
  runtimeStatus: 'idle' | 'checking' | 'ready' | 'missing' | 'installing' | 'error'
  runtimeError: UserFacingError | null
}

export const settingsInitialState: SettingsSlice = {
  settings: null,
  runtimeDependencies: null,
  runtimeStatus: 'idle',
  runtimeError: null,
}
