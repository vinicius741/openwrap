import type { StateCreator } from 'zustand'
import type { ProfileDetail, ProfileSummary } from '../../types/ipc'

export type ProfileSlice = {
  profiles: ProfileSummary[]
  selectedProfileId: string | null
  selectedProfile: ProfileDetail | null
  setProfiles: (profiles: ProfileSummary[]) => void
  setSelectedProfileId: (id: string | null) => void
  setSelectedProfile: (profile: ProfileDetail | null) => void
  updateProfileCredentialFlag: (profileId: string, hasSavedCredentials: boolean) => void
}

export const createProfileSlice: StateCreator<ProfileSlice, [], [], ProfileSlice> = (set) => ({
  profiles: [],
  selectedProfileId: null,
  selectedProfile: null,

  setProfiles: (profiles) => set({ profiles }),

  setSelectedProfileId: (selectedProfileId) => set({ selectedProfileId }),

  setSelectedProfile: (selectedProfile) => set({ selectedProfile }),

  updateProfileCredentialFlag: (profileId, hasSavedCredentials) =>
    set((state) => ({
      profiles: state.profiles.map((profile) =>
        profile.id === profileId
          ? { ...profile, has_saved_credentials: hasSavedCredentials }
          : profile,
      ),
      selectedProfile: state.selectedProfile
        ? {
            ...state.selectedProfile,
            profile: {
              ...state.selectedProfile.profile,
              has_saved_credentials:
                state.selectedProfile.profile.id === profileId
                  ? hasSavedCredentials
                  : state.selectedProfile.profile.has_saved_credentials,
            },
          }
        : null,
    })),
})
