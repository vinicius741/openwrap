import type { ConnectionSnapshot, ProfileDetail } from '../../../types/ipc'
import { StatusBadge } from '../../../components/StatusBadge'

interface ConnectionSummaryProps {
  connection: ConnectionSnapshot | undefined
  selectedProfile: ProfileDetail | null | undefined
}

export function ConnectionSummary({ connection, selectedProfile }: ConnectionSummaryProps) {
  return (
    <div className="connection-summary">
      <div>
        <p className="eyebrow">Connection</p>
        <h3>{selectedProfile?.profile.remote_summary || 'No remote'}</h3>
      </div>
      <StatusBadge state={connection?.state} />
    </div>
  )
}
