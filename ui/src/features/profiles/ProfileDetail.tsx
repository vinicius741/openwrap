import { EmptyState } from '../../components/EmptyState'
import { ConnectionPanel } from '../connection/ConnectionPanel'
import { LogPane } from '../logs/LogPane'
import { useAppStore } from '../../store/appStore'

export function ProfileDetail() {
  const profile = useAppStore((state) => state.selectedProfile)

  if (!profile) {
    return <EmptyState title="No profile selected" detail="Import a profile and select it from the sidebar." />
  }

  return (
    <div className="profile-detail">
      <header className="profile-header">
        <div>
          <p className="eyebrow">Managed profile</p>
          <h2>{profile.profile.name}</h2>
          <p className="profile-subtitle">{profile.profile.remote_summary || 'Remote summary unavailable'}</p>
        </div>
      </header>
      <ConnectionPanel />
      <section className="detail-grid">
        <article className="detail-card">
          <h3>Validation</h3>
          <ul className="finding-list">
            {profile.findings.length ? (
              profile.findings.map((finding) => (
                <li key={`${finding.directive}-${finding.line}`}>
                  <strong>{finding.directive}</strong> on line {finding.line}: {finding.message}
                </li>
              ))
            ) : (
              <li>No warnings or blocked directives were stored for this profile.</li>
            )}
          </ul>
        </article>
        <article className="detail-card">
          <h3>Managed assets</h3>
          <ul className="asset-list">
            {profile.assets.map((asset) => (
              <li key={asset.id}>
                <strong>{asset.kind}</strong>
                <span>{asset.relative_path}</span>
              </li>
            ))}
          </ul>
        </article>
      </section>
      <LogPane />
    </div>
  )
}

