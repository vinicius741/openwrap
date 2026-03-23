import { create } from 'zustand'

import type { ImportWarningState } from '../types/domain'
import type {
  ConnectionSnapshot,
  CredentialPrompt,
  LogEntry,
  OpenVpnDetection,
  ProfileDetail,
  ProfileSummary,
  Settings,
  UserFacingError,
} from '../types/ipc'

import {
  createProfileSlice,
  createConnectionSlice,
  createSettingsSlice,
  createImportSlice,
  createErrorSlice,
  type ProfileSlice,
  type ConnectionSlice,
  type SettingsSlice,
  type ImportSlice,
  type ErrorSlice,
  type UiLogEntry,
} from './slices'

import {
  createLoadInitialAction,
  createSelectProfileAction,
  createRefreshSelectedProfileAction,
  createRefreshProfilesAction,
  createDeleteProfileAction,
  createUpdateSelectedProfileDnsPolicyAction,
  createBeginImportAction,
  createApproveImportWarningsAction,
  createConnectSelectedAction,
  createDisconnectAction,
  createSubmitCredentialsAction,
  createSaveSettingsAction,
} from './actions'

/**
 * The combined state of all slices plus async actions.
 */
type AppStore = ProfileSlice &
  ConnectionSlice &
  SettingsSlice &
  ImportSlice &
  ErrorSlice & {
    // Async actions (delegated to action creators)
    loadInitial: () => Promise<void>
    selectProfile: (profileId: string) => Promise<void>
    refreshSelectedProfile: () => Promise<void>
    refreshProfiles: () => Promise<void>
    deleteProfile: (profileId: string) => Promise<void>
    updateSelectedProfileDnsPolicy: (
      dnsPolicy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly'
    ) => Promise<void>
    beginImport: (filePath: string) => Promise<void>
    approveImportWarnings: () => Promise<void>
    connectSelected: () => Promise<void>
    disconnect: () => Promise<void>
    submitCredentials: (input: {
      username: string
      password: string
      rememberInKeychain: boolean
    }) => Promise<void>
    saveSettings: (
      openvpnPathOverride: string | null,
      verboseLogging: boolean
    ) => Promise<void>
  }

/**
 * Create the combined store with all slices and actions.
 */
export const useAppStore = create<AppStore>((...args) => {
  const [set, get] = args

  // Create slices
  const profileSlice = createProfileSlice(...args)
  const connectionSlice = createConnectionSlice(...args)
  const settingsSlice = createSettingsSlice(...args)
  const importSlice = createImportSlice(...args)
  const errorSlice = createErrorSlice(...args)

  // Wrap setConnection to also clear error
  const originalSetConnection = connectionSlice.setConnection
  const setConnectionWithErrorClear: ConnectionSlice['setConnection'] = (snapshot) => {
    originalSetConnection(snapshot)
    set({ error: null })
  }

  // Wrap setCredentialPrompt to also clear error
  const originalSetCredentialPrompt = connectionSlice.setCredentialPrompt
  const setCredentialPromptWithErrorClear: ConnectionSlice['setCredentialPrompt'] = (prompt) => {
    originalSetCredentialPrompt(prompt)
    set({ error: null })
  }

  return {
    // Slice state
    ...profileSlice,
    ...connectionSlice,
    ...settingsSlice,
    ...importSlice,
    ...errorSlice,

    // Override wrapped methods
    setConnection: setConnectionWithErrorClear,
    setCredentialPrompt: setCredentialPromptWithErrorClear,

    // Async actions (delegated to action creators)
    loadInitial: createLoadInitialAction(get, set),
    selectProfile: createSelectProfileAction(get, set),
    refreshSelectedProfile: createRefreshSelectedProfileAction(get, set),
    refreshProfiles: createRefreshProfilesAction(get, set),
    deleteProfile: createDeleteProfileAction(get, set),
    updateSelectedProfileDnsPolicy: createUpdateSelectedProfileDnsPolicyAction(get, set),
    beginImport: createBeginImportAction(get, set),
    approveImportWarnings: createApproveImportWarningsAction(get, set),
    connectSelected: createConnectSelectedAction(get, set),
    disconnect: createDisconnectAction(get, set),
    submitCredentials: createSubmitCredentialsAction(get, set),
    saveSettings: createSaveSettingsAction(get, set),
  }
})

// Re-export UiLogEntry for backward compatibility
export type { UiLogEntry }
