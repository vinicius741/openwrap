import { connectProfile, disconnectProfile, submitCredentials as apiSubmitCredentials } from '../../features/connection/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { AppStoreApi } from '../createAppStore'

export function createConnectSelectedAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async () => {
    const profileId = get().selectedProfileId
    if (!profileId) {
      return
    }

    try {
      get().clearLogs()
      const snapshot = await connectProfile(profileId)
      set({ connection: snapshot, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createDisconnectAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async () => {
    try {
      const snapshot = await disconnectProfile()
      set({ connection: snapshot, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createSubmitCredentialsAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async (input: {
    username: string
    password: string
    rememberInKeychain: boolean
  }) => {
    const prompt = get().pendingCredentialPrompt
    if (!prompt) {
      return
    }

    try {
      const snapshot = await apiSubmitCredentials({
        profileId: prompt.profile_id,
        username: input.username,
        password: input.password,
        rememberInKeychain: input.rememberInKeychain,
      })
      set((state) => ({
        connection: snapshot,
        pendingCredentialPrompt: null,
        profiles: state.profiles.map((profile) =>
          profile.id === prompt.profile_id
            ? {
                ...profile,
                has_saved_credentials: input.rememberInKeychain,
              }
            : profile,
        ),
        selectedProfile: state.selectedProfile
          ? {
              ...state.selectedProfile,
              profile: {
                ...state.selectedProfile.profile,
                has_saved_credentials:
                  state.selectedProfile.profile.id === prompt.profile_id
                    ? input.rememberInKeychain
                    : state.selectedProfile.profile.has_saved_credentials,
              },
            }
          : null,
        error: null,
      }))
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}
