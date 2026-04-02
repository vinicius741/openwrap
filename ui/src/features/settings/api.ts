import { invokeCommand } from '../../lib/tauri'
import type { HelperStatus, OpenVpnDetection, Settings } from '../../types/ipc'

export async function getSettings() {
  return invokeCommand<Settings>('get_settings')
}

export async function updateSettings(patch: {
  openvpnPathOverride: string | null
  verboseLogging: boolean
}) {
  return invokeCommand<Settings>('update_settings', { patch })
}

export async function detectOpenVpn() {
  return invokeCommand<OpenVpnDetection>('detect_openvpn')
}

export async function checkHelperStatus() {
  return invokeCommand<HelperStatus>('check_helper_status')
}

export async function installHelper() {
  return invokeCommand<HelperStatus>('install_helper')
}
