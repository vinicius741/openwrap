import { invoke } from '@tauri-apps/api/core'

import type { UserFacingError } from '../types/ipc'

type CommandErrorPayload = {
  message?: string
  user_facing?: UserFacingError
}

export async function invokeCommand<T>(command: string, args?: Record<string, unknown>) {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw normalizeCommandError(error)
  }
}

export function normalizeCommandError(error: unknown): UserFacingError {
  if (typeof error === 'object' && error !== null) {
    const payload = error as CommandErrorPayload
    if (payload.user_facing) {
      return payload.user_facing
    }
    if ('code' in error && 'title' in error && 'message' in error) {
      return error as UserFacingError
    }
  }

  return {
    code: 'command_failed',
    title: 'Operation failed',
    message: typeof error === 'string' ? error : 'OpenWrap could not complete the requested action.',
    suggested_fix: null,
    details_safe: null,
  }
}
