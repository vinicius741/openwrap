import { invokeCommand } from '../../lib/tauri'
import type { RuntimeDependencies, Settings } from '../../types/ipc'

export function getRuntimeDependencies() {
  return invokeCommand<RuntimeDependencies>('get_runtime_dependencies')
}

export function installRuntimeDependencies() {
  return invokeCommand<RuntimeDependencies>('install_runtime_dependencies')
}

export async function getSettings() {
  return invokeCommand<Settings>('get_settings')
}

export async function updateSettings(patch: {
  openvpnPathOverride: string | null
  verboseLogging: boolean
}) {
  return invokeCommand<Settings>('update_settings', { patch })
}
