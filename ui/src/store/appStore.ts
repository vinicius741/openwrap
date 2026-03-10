import { create } from 'zustand'

import { connectProfile, disconnectProfile, getConnectionState, getRecentLogs, submitCredentials } from '../features/connection/api'
import { getLastSelectedProfile, getProfile, importProfile, listProfiles, setLastSelectedProfile } from '../features/profiles/api'
import { detectOpenVpn, getSettings, updateSettings } from '../features/settings/api'
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
import { normalizeCommandError } from '../lib/tauri'

type AppStore = {
  profiles: ProfileSummary[]
  selectedProfileId: string | null
  selectedProfile: ProfileDetail | null
  connection: ConnectionSnapshot | null
  logs: LogEntry[]
  pendingCredentialPrompt: CredentialPrompt | null
  settings: Settings | null
  detection: OpenVpnDetection | null
  importWarning: ImportWarningState | null
  error: UserFacingError | null
  loadInitial: () => Promise<void>
  selectProfile: (profileId: string) => Promise<void>
  refreshProfiles: () => Promise<void>
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
  appendLog: (entry: LogEntry) => void
  setCredentialPrompt: (prompt: CredentialPrompt | null) => void
  setDetection: (detection: OpenVpnDetection) => void
  setError: (error: UserFacingError | null) => void
  clearImportWarning: () => void
  saveSettings: (openvpnPathOverride: string | null) => Promise<void>
}

export const useAppStore = create<AppStore>((set, get) => ({
  profiles: [],
  selectedProfileId: null,
  selectedProfile: null,
  connection: null,
  logs: [],
  pendingCredentialPrompt: null,
  settings: null,
  detection: null,
  importWarning: null,
  error: null,

  loadInitial: async () => {
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
        logs,
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
  },

  selectProfile: async (profileId) => {
    try {
      const selectedProfile = await getProfile(profileId)
      set({
        selectedProfileId: profileId,
        selectedProfile,
        error: null,
      })
      await setLastSelectedProfile(profileId)
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  },

  refreshProfiles: async () => {
    try {
      const profiles = await listProfiles()
      set({ profiles, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  },

  beginImport: async (filePath) => {
    try {
      const response = await importProfile(filePath, false)
      if (response.report.status !== 'Imported') {
        set({
          importWarning: {
            draft: { filePath },
            response,
          },
          error: null,
        })
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
        await setLastSelectedProfile(response.profile.profile.id)
      }
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  },

  approveImportWarnings: async () => {
    const pending = get().importWarning
    if (!pending) {
      return
    }

    try {
      const response = await importProfile(pending.draft.filePath, true, pending.draft.displayName)
      if (response.report.status !== 'Imported') {
        set({
          importWarning: {
            draft: pending.draft,
            response,
          },
          error: null,
        })
        return
      }

      await get().refreshProfiles()
      set({ importWarning: null, error: null })

      if (response.profile) {
        set({
          selectedProfileId: response.profile.profile.id,
          selectedProfile: response.profile,
        })
        await setLastSelectedProfile(response.profile.profile.id)
      }
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  },

  connectSelected: async () => {
    const profileId = get().selectedProfileId
    if (!profileId) {
      return
    }

    try {
      const snapshot = await connectProfile(profileId)
      set({ connection: snapshot, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  },

  disconnect: async () => {
    try {
      const snapshot = await disconnectProfile()
      set({ connection: snapshot, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  },

  submitCredentials: async ({ username, password, rememberInKeychain }) => {
    const prompt = get().pendingCredentialPrompt
    if (!prompt) {
      return
    }

    try {
      const snapshot = await submitCredentials({
        profileId: prompt.profile_id,
        username,
        password,
        rememberInKeychain,
      })
      set((state) => ({
        connection: snapshot,
        pendingCredentialPrompt: null,
        selectedProfile: state.selectedProfile
          ? {
              ...state.selectedProfile,
              profile: {
                ...state.selectedProfile.profile,
                has_saved_credentials:
                  state.selectedProfile.profile.id === prompt.profile_id
                    ? rememberInKeychain || state.selectedProfile.profile.has_saved_credentials
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

  setConnection: (connection) => set({ connection, error: null }),

  setDnsObservation: (dnsObservation) =>
    set((state) => ({
      connection: state.connection
        ? {
            ...state.connection,
            dns_observation: dnsObservation,
          }
        : state.connection,
    })),

  appendLog: (entry) =>
    set((state) => ({
      logs: [...state.logs.slice(-399), entry],
    })),

  setCredentialPrompt: (pendingCredentialPrompt) => set({ pendingCredentialPrompt }),

  setDetection: (detection) => set({ detection }),

  setError: (error) => set({ error }),

  clearImportWarning: () => set({ importWarning: null }),

  saveSettings: async (openvpnPathOverride) => {
    try {
      const [settings, detection] = await Promise.all([
        updateSettings(openvpnPathOverride),
        detectOpenVpn(),
      ])
      set({ settings, detection, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  },
}))
