import type { ConnectionSnapshot } from '../../../types/ipc'
import { isConnected } from '../model/status'

interface ConnectionControlsProps {
  connection: ConnectionSnapshot | null | undefined
  onConnect: () => void
  onDisconnect: () => void
}

export function ConnectionControls({ connection, onConnect, onDisconnect }: ConnectionControlsProps) {
  const connected = isConnected(connection)

  return (
    <div className="connection-controls">
      <div className="button-group">
        <button className="action-button action-primary action-connect" disabled={connected} onClick={() => void onConnect()} type="button">
          Connect
        </button>
        <button className="action-button action-secondary" disabled={!connected} onClick={() => void onDisconnect()} type="button">
          Disconnect
        </button>
      </div>
    </div>
  )
}
