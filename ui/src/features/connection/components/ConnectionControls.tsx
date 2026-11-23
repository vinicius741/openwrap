import type { ConnectionSnapshot } from '../../../types/ipc'
import { canConnect, canDisconnect } from '../model/status'

interface ConnectionControlsProps {
  connection: ConnectionSnapshot | null | undefined
  onConnect: () => void
  onDisconnect: () => void
}

export function ConnectionControls({ connection, onConnect, onDisconnect }: ConnectionControlsProps) {

  return (
    <div className="connection-controls">
      <div className="button-group">
        <button className="action-button action-primary action-connect" disabled={!canConnect(connection)} onClick={() => void onConnect()} type="button">
          Connect
        </button>
        <button className="action-button action-secondary" disabled={!canDisconnect(connection)} onClick={() => void onDisconnect()} type="button">
          Disconnect
        </button>
      </div>
    </div>
  )
}
