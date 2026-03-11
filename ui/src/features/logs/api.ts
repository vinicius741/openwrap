import { invokeCommand } from '../../lib/tauri'
import { getRecentLogs } from '../connection/api'

export { getRecentLogs }

export async function revealConnectionLogInFinder() {
  return invokeCommand<void>('reveal_connection_log_in_finder')
}
