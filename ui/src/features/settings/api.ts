import { invoke } from '@tauri-apps/api/core'

import type { OpenVpnDetection, Settings } from '../../types/ipc'

export async function getSettings() {
  return invoke<Settings>('get_settings')
}

export async function updateSettings(openvpnPathOverride: string | null) {
  return invoke<Settings>('update_settings', {
    patch: { openvpnPathOverride },
  })
}

export async function detectOpenVpn() {
  return invoke<OpenVpnDetection>('detect_openvpn')
}
