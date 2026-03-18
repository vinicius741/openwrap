import { memo } from 'react'

import type { UiLogEntry } from '../store/appStore'

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
})

const LogLine = memo(function LogLine({ log }: { log: UiLogEntry }) {
  return (
    <div className={`log-line log-${log.level.toLowerCase()}`}>
      <span className="log-meta">{timeFormatter.format(new Date(log.ts))}</span>
      <span className="log-stream">{log.stream}</span>
      <span className="log-message">{log.message}</span>
    </div>
  )
})

export const LogViewer = memo(function LogViewer({ logs }: { logs: UiLogEntry[] }) {
  if (!logs.length) {
    return <div className="log-empty">No logs yet.</div>
  }

  return (
    <div className="log-viewer">
      {logs.map((log) => (
        <LogLine key={log.id} log={log} />
      ))}
    </div>
  )
})
