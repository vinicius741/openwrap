import type { LogEntry } from '../types/ipc'

export function LogViewer({ logs }: { logs: LogEntry[] }) {
  if (!logs.length) {
    return <div className="log-empty">No logs yet.</div>
  }

  return (
    <div className="log-viewer">
      {logs.map((log, index) => (
        <div key={`${log.ts}-${index}`} className={`log-line log-${log.level.toLowerCase()}`}>
          <span className="log-meta">{new Date(log.ts).toLocaleTimeString()}</span>
          <span className="log-stream">{log.stream}</span>
          <span className="log-message">{log.message}</span>
        </div>
      ))}
    </div>
  )
}

