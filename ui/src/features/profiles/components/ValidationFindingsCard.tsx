import type { ValidationFinding } from '../../../types/ipc'

interface ValidationFindingsCardProps {
  findings: ValidationFinding[]
}

export function ValidationFindingsCard({ findings }: ValidationFindingsCardProps) {
  return (
    <article className="detail-card">
      <h3>Validation</h3>
      <ul className="finding-list">
        {findings.length ? (
          findings.map((finding) => (
            <li key={`${finding.directive}-${finding.line}`}>
              <strong>{finding.directive}</strong> on line {finding.line}: {finding.message}
            </li>
          ))
        ) : (
          <li>No warnings or blocked directives were stored for this profile.</li>
        )}
      </ul>
    </article>
  )
}
