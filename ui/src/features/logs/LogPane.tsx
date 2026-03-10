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
        <span className="log-count">{logs.length} lines</span>
      </div>
      <div className="dns-observation">
        <strong>DNS behavior:</strong>{' '}
        {(connection?.dns_observation.config_requested ?? []).join(', ') || 'No DNS directives observed in config'}
      </div>
      <LogViewer logs={logs} />
    </section>
  )
}

