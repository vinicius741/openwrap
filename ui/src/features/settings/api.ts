import { invokeCommand } from '../../lib/tauri'
import type { Settings } from '../../types/ipc'

export async function getSettings() {
  return invokeCommand<Settings>('get_settings')
}

export async function updateSettings(patch: {
  openvpnPathOverride: string | null
  verboseLogging: boolean
}) {
  return invokeCommand<Settings>('update_settings', { patch })
}
