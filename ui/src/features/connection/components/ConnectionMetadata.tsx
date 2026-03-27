import type { ConnectionSnapshot, ProfileDetail } from '../../../types/ipc'

interface ConnectionMetadataProps {
  connection: ConnectionSnapshot | undefined
  selectedProfile: ProfileDetail | null | undefined
}

export function ConnectionMetadata({ connection, selectedProfile }: ConnectionMetadataProps) {
  return (
    <div className="connection-metadata-compact">
      <span className="metadata-pair">
        <span className="metadata-label">PID</span>
        <span className="metadata-value">{connection?.pid ?? 'Not started'}</span>
      </span>
      <span className="metadata-sep" aria-hidden="true">·</span>
      <span className="metadata-pair">
        <span className="metadata-label">DNS</span>
        <span className="metadata-value">{connection?.dns_observation.effective_mode ?? 'ObserveOnly'}</span>
      </span>
      <span className="metadata-sep" aria-hidden="true">·</span>
      <span className="metadata-pair">
        <span className="metadata-label">Username</span>
        <span className="metadata-value">{selectedProfile?.profile.has_saved_credentials ? 'Saved' : 'None'}</span>
      </span>
    </div>
  )
}
