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

export async function deleteProfile(profileId: string) {
  return invokeCommand<void>('delete_profile', { profileId })
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

export async function updateProfileDnsPolicy(
  profileId: string,
  dnsPolicy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly',
) {
  return invokeCommand<ProfileDetail>('update_profile_dns_policy', {
    request: {
      profileId,
      dnsPolicy,
    },
  })
}

export async function configureGeneratedPasswordProfile(input: {
  profileId: string
  username: string
  pin: string
  totpSecret: string
}) {
  return invokeCommand<ProfileDetail>('configure_generated_password_profile', input)
}

export async function clearGeneratedPasswordProfile(profileId: string) {
  return invokeCommand<ProfileDetail>('clear_generated_password_profile', {
    profileId,
  })
}
