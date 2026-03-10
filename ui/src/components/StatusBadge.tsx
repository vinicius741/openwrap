import type { ConnectionState } from '../types/ipc'

export function StatusBadge({ state }: { state: ConnectionState | null | undefined }) {
  const label = state ?? 'idle'
  return <span className={`status-badge status-${label}`}>{label.replace(/_/g, ' ')}</span>
}

