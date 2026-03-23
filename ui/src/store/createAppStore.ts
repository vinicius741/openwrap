import { create } from 'zustand'
import { profileInitialState } from './slices/profileSlice'
import { connectionInitialState, type UiLogEntry } from './slices/connectionSlice'
import { settingsInitialState } from './slices/settingsSlice'
import { importInitialState } from './slices/importSlice'
import { loadInitial } from './actions/loadInitial'
import {
  selectProfile,
  refreshSelectedProfile,
  refreshProfiles,
  deleteProfileAction,
  updateDnsPolicy,
} from './actions/profileActions'
import { connectSelected, disconnect, submitCredentialsAction } from './actions/connectionActions'
import {
  reduceConnection,
  reduceDnsObservation,
  reduceAppendLogs,
} from './reducers/connectionEvents'
import { importProfile } from '../features/profiles/api'
import { updateSettings, detectOpenVpn } from '../features/settings/api'
import { normalizeCommandError } from '../lib/tauri'
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

type AppStore = {
  profiles: ProfileSummary[]
  selectedProfileId: string | null
  selectedProfile: ProfileDetail | null
  connection: ConnectionSnapshot | null
  logs: UiLogEntry[]
  nextLogId: number
  pendingCredentialPrompt: CredentialPrompt | null
  settings: Settings | null
  detection: OpenVpnDetection | null
  importWarning: ImportWarningState | null
  error: UserFacingError | null
  loadInitial: () => Promise<void>
  selectProfile: (profileId: string) => Promise<void>
  refreshSelectedProfile: () => Promise<void>
  refreshProfiles: () => Promise<void>
  deleteProfile: (profileId: string) => Promise<void>
  updateSelectedProfileDnsPolicy: (dnsPolicy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly') => Promise<void>
  beginImport: (filePath: string) => Promise<void>
  approveImportWarnings: () => Promise<void>
  connectSelected: () => Promise<void>
  disconnect: () => Promise<void>
  submitCredentials: (input: {
    username: string
    password: string
    rememberInKeychain: boolean
  }) => Promise<void>
  setConnection: (snapshot: ConnectionSnapshot) => void
  setDnsObservation: (dnsObservation: ConnectionSnapshot['dns_observation']) => void
  appendLogs: (entries: LogEntry[]) => void
  clearLogs: () => void
  setCredentialPrompt: (prompt: CredentialPrompt | null) => void
  setDetection: (detection: OpenVpnDetection) => void
  setError: (error: UserFacingError | null) => void
  clearImportWarning: () => void
  saveSettings: (openvpnPathOverride: string | null, verboseLogging: boolean) => Promise<void>
}

export const createAppStore = () => {
  return create<AppStore>((set, get) => ({
    ...profileInitialState,
    ...connectionInitialState,
    ...settingsInitialState,
    ...importInitialState,
    error: null,

    loadInitial: async () => {
      try {
        const result = await loadInitial()
        set(result)
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    selectProfile: async (profileId) => {
      try {
        const selectedProfile = await selectProfile(profileId)
        set({ selectedProfileId: profileId, selectedProfile, error: null })
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    refreshSelectedProfile: async () => {
      const selectedProfileId = get().selectedProfileId
      if (!selectedProfileId) return
      try {
        const selectedProfile = await refreshSelectedProfile(selectedProfileId)
        set({ selectedProfile, error: null })
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    refreshProfiles: async () => {
      try {
        const profiles = await refreshProfiles()
        set({ profiles, error: null })
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    deleteProfile: async (profileId) => {
      try {
        await deleteProfileAction(profileId)
        await get().refreshProfiles()
        const { selectedProfileId, profiles } = get()
        if (selectedProfileId === profileId) {
          const nextProfileId = profiles.length > 0 ? profiles[0].id : null
          if (nextProfileId) {
            await get().selectProfile(nextProfileId)
          } else {
            set({ selectedProfileId: null, selectedProfile: null })
          }
        }
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    updateSelectedProfileDnsPolicy: async (dnsPolicy) => {
      const selectedProfile = get().selectedProfile
      if (!selectedProfile) return
      try {
        const updatedProfile = await updateDnsPolicy(selectedProfile.profile.id, dnsPolicy)
        set({ selectedProfile: updatedProfile, error: null })
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    beginImport: async (filePath) => {
      try {
        const response = await importProfile(filePath, false)
        if (response.report.status !== 'Imported') {
          set({ importWarning: { draft: { filePath }, response }, error: null })
          return
        }
        await get().refreshProfiles()
        if (response.profile) {
          set({
            selectedProfileId: response.profile.profile.id,
            selectedProfile: response.profile,
            importWarning: null,
            error: null,
          })
        }
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    approveImportWarnings: async () => {
      const pending = get().importWarning
      if (!pending) return
      try {
        const response = await importProfile(pending.draft.filePath, true, pending.draft.displayName)
        if (response.report.status !== 'Imported') {
          set({ importWarning: { draft: pending.draft, response }, error: null })
          return
        }
        await get().refreshProfiles()
        set({ importWarning: null, error: null })
        if (response.profile) {
          set({
            selectedProfileId: response.profile.profile.id,
            selectedProfile: response.profile,
          })
        }
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    connectSelected: async () => {
      const profileId = get().selectedProfileId
      if (!profileId) return
      try {
        get().clearLogs()
        const connection = await connectSelected(profileId)
        set({ connection, error: null })
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    disconnect: async () => {
      try {
        const connection = await disconnect()
        set({ connection, error: null })
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    submitCredentials: async ({ username, password, rememberInKeychain }) => {
      const prompt = get().pendingCredentialPrompt
      if (!prompt) return
      try {
        const connection = await submitCredentialsAction({
          profileId: prompt.profile_id,
          username,
          password,
          rememberInKeychain,
        })
        set((state) => ({
          connection,
          pendingCredentialPrompt: null,
          profiles: state.profiles.map((profile) =>
            profile.id === prompt.profile_id
              ? { ...profile, has_saved_credentials: rememberInKeychain }
              : profile,
          ),
          selectedProfile: state.selectedProfile
            ? {
                ...state.selectedProfile,
                profile: {
                  ...state.selectedProfile.profile,
                  has_saved_credentials:
                    state.selectedProfile.profile.id === prompt.profile_id
                      ? rememberInKeychain
                      : state.selectedProfile.profile.has_saved_credentials,
                },
              }
            : null,
          error: null,
        }))
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },

    setConnection: (snapshot) => {
      const { logs, nextLogId } = get()
      const result = reduceConnection(logs, nextLogId, snapshot)
      set({ ...result, error: null })
    },

    setDnsObservation: (dnsObservation) => {
      const { connection } = get()
      const result = reduceDnsObservation(connection, dnsObservation)
      set(result)
    },

    appendLogs: (entries) => {
      const { logs, nextLogId } = get()
      const result = reduceAppendLogs(logs, nextLogId, entries)
      set(result)
    },

    clearLogs: () => set({ logs: [], nextLogId: 0 }),

    setCredentialPrompt: (pendingCredentialPrompt) => set({ pendingCredentialPrompt, error: null }),

    setDetection: (detection) => set({ detection }),

    setError: (error) => set({ error }),

    clearImportWarning: () => set({ importWarning: null }),

    saveSettings: async (openvpnPathOverride, verboseLogging) => {
      try {
        const [settings, detection] = await Promise.all([
          updateSettings({ openvpnPathOverride, verboseLogging }),
          detectOpenVpn(),
        ])
        set({ settings, detection, error: null })
      } catch (error) {
        set({ error: normalizeCommandError(error) })
      }
    },
  }))
}
