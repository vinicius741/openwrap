import type { ProfileSummary, ProfileDetail } from '../../types/ipc'

export type ProfileSlice = {
  profiles: ProfileSummary[]
  selectedProfileId: string | null
  selectedProfile: ProfileDetail | null
}

export const profileInitialState: ProfileSlice = {
  profiles: [],
  selectedProfileId: null,
  selectedProfile: null,
}
