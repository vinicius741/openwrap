import type { ConnectionSnapshot, DnsObservation } from '../../../types/ipc'

/** True while a session exists whose DNS setup was fixed when it started. */
export function hasActiveSession(connection: ConnectionSnapshot | null | undefined): boolean {
  if (!connection) {
    return false
  }
  return (
    connection.state !== 'idle' &&
    connection.state !== 'error' &&
    connection.state !== 'disconnecting'
  )
}

export function isConnected(connection: ConnectionSnapshot | null | undefined): boolean {
  if (!connection) {
    return false
  }
  return (
    connection.state === 'connected' ||
    connection.state === 'connecting' ||
    connection.state === 'reconnecting' ||
    connection.state === 'awaiting_credentials'
  )
}

export function getDnsStatusMessage(observation: DnsObservation | undefined): string | null {
  if (!observation) {
    return null
  }

  if (observation.restore_status === 'restore_failed') {
    return 'DNS restore failed; OpenWrap will retry reconciliation on next launch or when you reconnect.'
  }

  if (observation.restore_status === 'pending_reconcile') {
    return 'DNS restore is pending reconciliation; OpenWrap will retry on the next launch or reconnect.'
  }

  if (observation.effective_mode === 'ScopedResolvers') {
    // The backend reports this case as a warning that starts with this text.
    const missingDomains = observation.warnings.find((warning) =>
      warning.startsWith('No VPN domains are listed'),
    )
    return (
      missingDomains ??
      'OpenWrap uses VPN DNS only for the domains listed for this profile. All other DNS stays on your normal network.'
    )
  }

  if (observation.effective_mode === 'GlobalOverride') {
    return 'Full DNS override is active. OpenWrap is routing all system DNS through the VPN.'
  }

  return null
}
