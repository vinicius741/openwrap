import type { StateCreator } from 'zustand'
import type { ConnectionSnapshot, CredentialPrompt, LogEntry } from '../../types/ipc'

export type UiLogEntry = LogEntry & {
  id: number
}

export type ConnectionSlice = {
  connection: ConnectionSnapshot | null
  logs: UiLogEntry[]
  nextLogId: number
  pendingCredentialPrompt: CredentialPrompt | null
  setConnection: (snapshot: ConnectionSnapshot) => void
  setDnsObservation: (dnsObservation: ConnectionSnapshot['dns_observation']) => void
  appendLogs: (entries: LogEntry[]) => void
  clearLogs: () => void
  setCredentialPrompt: (prompt: CredentialPrompt | null) => void
}

export const createConnectionSlice: StateCreator<ConnectionSlice, [], [], ConnectionSlice> = (set) => ({
  connection: null,
  logs: [],
  nextLogId: 0,
  pendingCredentialPrompt: null,

  setConnection: (connection) =>
    set((state) => ({
      connection,
      logs:
        connection.state === 'validating_profile' && connection.retry_count === 0
          ? []
          : state.logs,
      nextLogId:
        connection.state === 'validating_profile' && connection.retry_count === 0
          ? 0
          : state.nextLogId,
    })),

  setDnsObservation: (dnsObservation) =>
    set((state) => ({
      connection: state.connection
        ? {
            ...state.connection,
            dns_observation: dnsObservation,
          }
        : state.connection,
    })),

  appendLogs: (entries) => {
    if (!entries.length) {
      return
    }

    set((state) => {
      const nextEntries = entries.map((entry, index) => ({
        ...entry,
        id: state.nextLogId + index + 1,
      }))

      return {
        logs: [...state.logs, ...nextEntries].slice(-400),
        nextLogId: state.nextLogId + entries.length,
      }
    })
  },

  clearLogs: () => set({ logs: [], nextLogId: 0 }),

  setCredentialPrompt: (pendingCredentialPrompt) => set({ pendingCredentialPrompt }),
})
