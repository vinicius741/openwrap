import type { ConnectionSnapshot, DnsObservation } from '../../../types/ipc'

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

  if (observation.auto_promoted_policy === 'FullOverride') {
    return 'OpenWrap auto-promoted this connection to full DNS override because the VPN did not provide split-DNS domains.'
  }

  if (observation.effective_mode === 'ScopedResolvers') {
    return 'Split DNS succeeded. OpenWrap is using VPN DNS only for configured domains.'
  }

  if (observation.effective_mode === 'GlobalOverride') {
    return 'Full DNS override is active. OpenWrap is routing all system DNS through the VPN.'
  }

  return null
}
