import { invokeCommand } from '../../lib/tauri'
import type {
  ImportProfileResponse,
  ProfileDetail,
  ProfileSummary,
} from '../../types/ipc'

export async function listProfiles() {
  return invokeCommand<ProfileSummary[]>('list_profiles')
}

export async function getProfile(profileId: string) {
  return invokeCommand<ProfileDetail>('get_profile', { profileId })
}

export async function importProfile(
  filePath: string,
  allowWarnings = false,
  displayName?: string,
) {
  return invokeCommand<ImportProfileResponse>('import_profile', {
    request: {
      filePath,
      displayName,
      allowWarnings,
    },
  })
}

export async function getLastSelectedProfile() {
  return invokeCommand<string | null>('get_last_selected_profile')
}

export async function setLastSelectedProfile(profileId: string | null) {
  return invokeCommand<void>('set_last_selected_profile', { profileId })
}
