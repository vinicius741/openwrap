import type { ConnectionSnapshot, CredentialPrompt, LogEntry } from '../../types/ipc'

/**
 * Event handlers for connection-related events from the backend.
 * These are pure functions that produce state updates.
 */
export function handleConnectionStateChanged(
  setConnection: (snapshot: ConnectionSnapshot) => void,
  setError: (error: null) => void,
  snapshot: ConnectionSnapshot,
): void {
  setConnection(snapshot)
  setError(null)
}

export function handleDnsObserved(
  setDnsObservation: (dnsObservation: ConnectionSnapshot['dns_observation']) => void,
  dnsObservation: ConnectionSnapshot['dns_observation'],
): void {
  setDnsObservation(dnsObservation)
}

export function handleLogLine(
  appendLogs: (entries: LogEntry[]) => void,
  entry: LogEntry,
): void {
  appendLogs([entry])
}

export function handleCredentialsRequested(
  setCredentialPrompt: (prompt: CredentialPrompt | null) => void,
  setError: (error: null) => void,
  prompt: CredentialPrompt,
): void {
  setCredentialPrompt(prompt)
  setError(null)
}
