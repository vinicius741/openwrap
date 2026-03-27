import { useState } from 'react'
import type { ValidationFinding, ManagedAsset } from '../../../types/ipc'

interface ProfileInfoProps {
  findings: ValidationFinding[]
  assets: ManagedAsset[]
}

export function ProfileInfo({ findings, assets }: ProfileInfoProps) {
  const [isOpen, setIsOpen] = useState(false)

  const hasContent = findings.length > 0 || assets.length > 0
  const summary = [
    findings.length > 0 ? `${findings.length} validation finding${findings.length !== 1 ? 's' : ''}` : null,
    assets.length > 0 ? `${assets.length} managed asset${assets.length !== 1 ? 's' : ''}` : null,
  ]
    .filter(Boolean)
    .join(' / ') || 'No validation findings or managed assets'

  return (
    <section className="profile-info-section">
      <button
        className="profile-info-toggle"
        onClick={() => setIsOpen((prev) => !prev)}
        type="button"
        aria-expanded={isOpen}
        aria-controls="profile-info-content"
      >
        <div>
          <p className="eyebrow">Profile info</p>
          <h3>Validation & assets</h3>
        </div>
        <div className="profile-info-summary">
          <span className="status-badge">{summary}</span>
          <svg
            className={`chevron ${isOpen ? 'chevron-open' : ''}`}
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </div>
      </button>

      {isOpen && hasContent && (
        <div className="profile-info-content" id="profile-info-content">
          {findings.length > 0 && (
            <div className="profile-info-group">
              <h4>Validation findings</h4>
              <ul className="info-list">
                {findings.map((finding) => (
                  <li key={`${finding.directive}-${finding.line}`}>
                    <strong>{finding.directive}</strong> on line {finding.line}: {finding.message}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {assets.length > 0 && (
            <div className="profile-info-group">
              <h4>Managed assets</h4>
              <ul className="info-list">
                {assets.map((asset) => (
                  <li key={asset.id}>
                    <strong>{asset.kind}</strong>
                    <span>{asset.relative_path}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </section>
  )
}
