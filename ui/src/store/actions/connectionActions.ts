import { connectProfile, disconnectProfile, submitCredentials } from '../../features/connection/api'
import type { ConnectionSnapshot } from '../../types/ipc'

export async function connectSelected(profileId: string): Promise<ConnectionSnapshot> {
  return connectProfile(profileId)
}

export async function disconnect(): Promise<ConnectionSnapshot> {
  return disconnectProfile()
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

