import type { RuntimeDependencies, Settings, UserFacingError } from '../../types/ipc'

export type SettingsSlice = {
  settings: Settings | null
  runtimeDependencies: RuntimeDependencies | null
  /** `broken` means OpenVPN is installed but cannot start. */
  runtimeStatus:
    | 'idle'
    | 'checking'
    | 'ready'
    | 'missing'
    | 'broken'
    | 'installing'
    | 'repairing'
    | 'error'
  runtimeError: UserFacingError | null
}

export const settingsInitialState: SettingsSlice = {
  settings: null,
  runtimeDependencies: null,
  runtimeStatus: 'idle',
  runtimeError: null,
}
