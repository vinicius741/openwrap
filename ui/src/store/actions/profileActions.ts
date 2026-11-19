import {
  clearGeneratedPasswordProfile,
  configureGeneratedPasswordProfile,
  deleteProfile,
  getProfile,
  listProfiles,
  setLastSelectedProfile,
  updateProfileDnsMatchDomains,
  updateProfileDnsPolicy,
} from '../../features/profiles/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { DnsPolicy, ProfileDetail, ProfileSummary, UserFacingError } from '../../types/ipc'

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
  dnsPolicy: DnsPolicy,
): Promise<ProfileDetail> {
  return updateProfileDnsPolicy(profileId, dnsPolicy)
}

export async function updateDnsMatchDomains(
  profileId: string,
  domains: string[],
): Promise<ProfileDetail> {
  return updateProfileDnsMatchDomains(profileId, domains)
}

export function updateDnsPolicyError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function configureGeneratedPassword(
  profileId: string,
  username: string,
  pin: string,
  totpSecret: string,
): Promise<ProfileDetail> {
  return configureGeneratedPasswordProfile({
    profileId,
    username,
    pin,
    totpSecret,
  })
}

export function configureGeneratedPasswordError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function clearGeneratedPassword(profileId: string): Promise<ProfileDetail> {
  return clearGeneratedPasswordProfile(profileId)
}

export function clearGeneratedPasswordError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}
