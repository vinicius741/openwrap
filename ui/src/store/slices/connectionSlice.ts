import type { ConnectionSnapshot, CredentialPrompt, LogEntry } from '../../types/ipc'

export type UiLogEntry = LogEntry & {
  id: number
}

export type ConnectionSlice = {
  connection: ConnectionSnapshot | null
  logs: UiLogEntry[]
  nextLogId: number
  pendingCredentialPrompt: CredentialPrompt | null
}

export const connectionInitialState: ConnectionSlice = {
  connection: null,
  logs: [],
  nextLogId: 0,
  pendingCredentialPrompt: null,
}
