import { invokeCommand } from '../../lib/tauri'
import type { OpenVpnDetection, Settings } from '../../types/ipc'

export async function getSettings() {
  return invokeCommand<Settings>('get_settings')
}

export async function updateSettings(openvpnPathOverride: string | null) {
  return invokeCommand<Settings>('update_settings', {
    patch: { openvpnPathOverride },
  })
}

export async function detectOpenVpn() {
  return invokeCommand<OpenVpnDetection>('detect_openvpn')
}
