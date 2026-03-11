import { normalizeCommandError } from '../../lib/tauri'
import { LogViewer } from '../../components/LogViewer'
import { revealConnectionLogInFinder } from './api'
import { useAppStore } from '../../store/appStore'

export function LogPane() {
  const logs = useAppStore((state) => state.logs)
  const connection = useAppStore((state) => state.connection)
  const setError = useAppStore((state) => state.setError)

  const handleRevealLog = async () => {
    try {
      await revealConnectionLogInFinder()
    } catch (error) {
      setError(normalizeCommandError(error))
    }
  }

  return (
    <section className="detail-card" id="connection-logs">
      <div className="section-heading log-pane-heading">
        <div>
          <p className="eyebrow">Logs</p>
          <h3>OpenVPN output</h3>
        </div>
        <div className="section-heading-actions">
          {connection?.log_file_path ? (
            <button className="action-button action-secondary action-small" onClick={() => void handleRevealLog()} type="button">
              Reveal log file
            </button>
          ) : null}
          <span className="status-badge" style={{ padding: '4px 10px', fontSize: '11px' }}>
            {logs.length} lines
          </span>
        </div>
      </div>
      {connection?.log_file_path ? (
        <div className="log-path-block">
          <p className="log-path-label">Latest saved failure log</p>
          <p className="log-file-path">{connection.log_file_path}</p>
        </div>
      ) : null}
      <div className="dns-observation" style={{ marginTop: '12px', fontSize: '13px' }}>
        <strong>DNS behavior:</strong>{' '}
        <span style={{ color: 'var(--text)' }}>
          {(connection?.dns_observation.config_requested ?? []).join(', ') || 'No DNS directives observed in config'}
        </span>
      </div>
      <LogViewer logs={logs} />
    </section>
  )
}
