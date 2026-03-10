import { create } from 'zustand'

import { connectProfile, disconnectProfile, getConnectionState, getRecentLogs, submitCredentials } from '../features/connection/api'
import { getProfile, importProfile, listProfiles, setLastSelectedProfile } from '../features/profiles/api'
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
} from '../types/ipc'

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
  errorMessage: string | null
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
  appendLog: (entry: LogEntry) => void
  setCredentialPrompt: (prompt: CredentialPrompt | null) => void
  setDetection: (detection: OpenVpnDetection) => void
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
  errorMessage: null,

  loadInitial: async () => {
    const [profiles, connection, logs, settings, detection] = await Promise.all([
      listProfiles(),
      getConnectionState(),
      getRecentLogs(),
      getSettings(),
      detectOpenVpn(),
    ])

    const selectedProfileId = profiles[0]?.id ?? null
    const selectedProfile = selectedProfileId ? await getProfile(selectedProfileId) : null

    set({
      profiles,
      selectedProfileId,
      selectedProfile,
      connection,
      logs,
      settings,
      detection,
      errorMessage: null,
    })

    if (selectedProfileId) {
      await setLastSelectedProfile(selectedProfileId)
    }
  },

  selectProfile: async (profileId) => {
    const selectedProfile = await getProfile(profileId)
    set({
      selectedProfileId: profileId,
      selectedProfile,
    })
    await setLastSelectedProfile(profileId)
  },

  refreshProfiles: async () => {
    const profiles = await listProfiles()
    set({ profiles })
  },

  beginImport: async (filePath) => {
    const response = await importProfile(filePath, false)
    if (response.report.status === 'NeedsApproval') {
      set({
        importWarning: {
          draft: { filePath },
          response,
        },
      })
      return
    }

    await get().refreshProfiles()
    if (response.profile) {
      set({
        selectedProfileId: response.profile.profile.id,
        selectedProfile: response.profile,
        importWarning: null,
      })
      await setLastSelectedProfile(response.profile.profile.id)
    }
  },

  approveImportWarnings: async () => {
    const pending = get().importWarning
    if (!pending) {
      return
    }

    const response = await importProfile(pending.draft.filePath, true, pending.draft.displayName)
    await get().refreshProfiles()
    set({ importWarning: null })

    if (response.profile) {
      set({
        selectedProfileId: response.profile.profile.id,
        selectedProfile: response.profile,
      })
      await setLastSelectedProfile(response.profile.profile.id)
    }
  },

  connectSelected: async () => {
    const profileId = get().selectedProfileId
    if (!profileId) {
      return
    }

    const snapshot = await connectProfile(profileId)
    set({ connection: snapshot })
  },

  disconnect: async () => {
    const snapshot = await disconnectProfile()
    set({ connection: snapshot })
  },

  submitCredentials: async ({ username, password, rememberInKeychain }) => {
    const prompt = get().pendingCredentialPrompt
    if (!prompt) {
      return
    }

    const snapshot = await submitCredentials({
      profileId: prompt.profile_id,
      username,
      password,
      rememberInKeychain,
    })
    set({
      connection: snapshot,
      pendingCredentialPrompt: null,
    })
  },

  setConnection: (connection) => set({ connection }),

  appendLog: (entry) =>
    set((state) => ({
      logs: [...state.logs.slice(-399), entry],
    })),

  setCredentialPrompt: (pendingCredentialPrompt) => set({ pendingCredentialPrompt }),

  setDetection: (detection) => set({ detection }),

  clearImportWarning: () => set({ importWarning: null }),

  saveSettings: async (openvpnPathOverride) => {
    const [settings, detection] = await Promise.all([
      updateSettings(openvpnPathOverride),
      detectOpenVpn(),
    ])
    set({ settings, detection })
  },
}))
