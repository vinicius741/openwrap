import { LogViewer } from '../../components/LogViewer'
import { useAppStore } from '../../store/appStore'

export function LogPane() {
  const logs = useAppStore((state) => state.logs)
  const connection = useAppStore((state) => state.connection)

  return (
    <section className="detail-card">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Logs</p>
          <h3>OpenVPN output</h3>
        </div>
        <span className="status-badge" style={{ padding: '4px 10px', fontSize: '11px' }}>
          {logs.length} lines
        </span>
      </div>
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

