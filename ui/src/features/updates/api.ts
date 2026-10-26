import { invokeCommand } from '../../lib/tauri'
import type { AppInfo, UpdateInfo } from '../../types/ipc'

export async function checkForUpdate() {
  return invokeCommand<UpdateInfo | null>('check_for_update')
}

export async function installUpdate() {
  return invokeCommand<void>('install_update')
}

export async function getAppInfo() {
  return invokeCommand<AppInfo>('get_app_info')
}
