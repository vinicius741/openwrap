import type { ConnectionSnapshot, CredentialPrompt, LogEntry, OpenVpnDetection, UserFacingError } from '../../types/ipc'
import type { UiLogEntry } from '../slices/connectionSlice'

export function reduceConnection(
  logs: UiLogEntry[],
  nextLogId: number,
  connection: ConnectionSnapshot | null,
): { logs: UiLogEntry[]; nextLogId: number; connection: ConnectionSnapshot | null } {
  return {
    logs:
      connection?.state === 'validating_profile' && connection.retry_count === 0
        ? []
        : logs,
    nextLogId:
      connection?.state === 'validating_profile' && connection.retry_count === 0
        ? 0
        : nextLogId,
    connection,
  }
}

export function reduceDnsObservation(
  connection: ConnectionSnapshot | null,
  dnsObservation: ConnectionSnapshot['dns_observation'],
): { connection: ConnectionSnapshot | null } {
  return {
    connection: connection
      ? { ...connection, dns_observation: dnsObservation }
      : connection,
  }
}

export function reduceAppendLogs(
  logs: UiLogEntry[],
  nextLogId: number,
  entries: LogEntry[],
): { logs: UiLogEntry[]; nextLogId: number } {
  if (!entries.length) {
    return { logs, nextLogId }
  }
  const nextEntries = entries.map((entry, index) => ({
    ...entry,
    id: nextLogId + index + 1,
  }))
  return {
    logs: [...logs, ...nextEntries].slice(-400),
    nextLogId: nextLogId + entries.length,
  }
}

export function reduceCredentialPrompt(
  pendingCredentialPrompt: CredentialPrompt | null,
): { pendingCredentialPrompt: CredentialPrompt | null } {
  return { pendingCredentialPrompt }
}

export function reduceDetection(detection: OpenVpnDetection): { detection: OpenVpnDetection } {
  return { detection }
}

export function reduceError(error: UserFacingError | null): { error: UserFacingError | null } {
  return { error }
}
