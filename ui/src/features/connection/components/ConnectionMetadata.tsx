import type { ConnectionSnapshot, ProfileDetail } from '../../../types/ipc'

interface ConnectionMetadataProps {
  connection: ConnectionSnapshot | undefined
  selectedProfile: ProfileDetail | null | undefined
}

export function ConnectionMetadata({ connection, selectedProfile }: ConnectionMetadataProps) {
  return (
    <div className="connection-metadata">
      <div className="metadata-item">
        <span className="metadata-label">PID</span>
        <strong className="metadata-value">{connection?.pid ?? 'Not started'}</strong>
      </div>
      <div className="metadata-item">
        <span className="metadata-label">DNS mode</span>
        <strong className="metadata-value">{connection?.dns_observation.effective_mode ?? 'ObserveOnly'}</strong>
      </div>
      <div className="metadata-item">
        <span className="metadata-label">Saved username</span>
        <strong className="metadata-value">{selectedProfile?.profile.has_saved_credentials ? 'Yes' : 'No'}</strong>
      </div>
    </div>
  )
}
