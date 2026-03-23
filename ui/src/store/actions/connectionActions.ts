import { connectProfile, disconnectProfile, submitCredentials } from '../../features/connection/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { ConnectionSnapshot, ProfileSummary, UserFacingError } from '../../types/ipc'

export async function connectSelected(profileId: string): Promise<ConnectionSnapshot> {
  return connectProfile(profileId)
}

export function connectSelectedError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function disconnect(): Promise<ConnectionSnapshot> {
  return disconnectProfile()
}

export function disconnectError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}

export async function submitCredentialsAction(input: {
  profileId: string
  username: string
  password: string
  rememberInKeychain: boolean
}): Promise<ConnectionSnapshot> {
  return submitCredentials({
    profileId: input.profileId,
    username: input.username,
    password: input.password,
    rememberInKeychain: input.rememberInKeychain,
  })
}

export function submitCredentialsError(error: unknown): { error: UserFacingError } {
  return { error: normalizeCommandError(error) }
}
