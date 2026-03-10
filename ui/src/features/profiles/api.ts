import { invoke } from '@tauri-apps/api/core'

import type {
  ImportProfileResponse,
  ProfileDetail,
  ProfileSummary,
} from '../../types/ipc'

export async function listProfiles() {
  return invoke<ProfileSummary[]>('list_profiles')
}

export async function getProfile(profileId: string) {
  return invoke<ProfileDetail>('get_profile', { profileId })
}

export async function importProfile(
  filePath: string,
  allowWarnings = false,
  displayName?: string,
) {
  return invoke<ImportProfileResponse>('import_profile', {
    request: {
      filePath,
      displayName,
      allowWarnings,
    },
  })
}

export async function setLastSelectedProfile(profileId: string | null) {
  return invoke<void>('set_last_selected_profile', { profileId })
}

