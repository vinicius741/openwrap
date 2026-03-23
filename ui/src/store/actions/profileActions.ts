import { getProfile, deleteProfile, listProfiles, setLastSelectedProfile, updateProfileDnsPolicy } from '../../features/profiles/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { ProfileDetail, ProfileSummary, UserFacingError } from '../../types/ipc'

export async function selectProfile(profileId: string): Promise<ProfileDetail> {
  const selectedProfile = await getProfile(profileId)
  await setLastSelectedProfile(profileId)
  return selectedProfile
}

export function selectProfileError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function refreshSelectedProfile(profileId: string): Promise<ProfileDetail> {
  return getProfile(profileId)
}

export function refreshSelectedProfileError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function refreshProfiles(): Promise<ProfileSummary[]> {
  return listProfiles()
}

export function refreshProfilesError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function deleteProfileAction(profileId: string): Promise<void> {
  await deleteProfile(profileId)
}

export function deleteProfileError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function updateDnsPolicy(
  profileId: string,
  dnsPolicy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly',
): Promise<ProfileDetail> {
  return updateProfileDnsPolicy(profileId, dnsPolicy)
}

export function updateDnsPolicyError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}
