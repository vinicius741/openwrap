export function EmptyState({
  title,
  detail,
}: {
  title: string
  detail: string
}) {
  return (
    <div className="empty-state">
      <div className="empty-state-content">
        <h2>{title}</h2>
        <p>{detail}</p>
      </div>
    </div>
  )
}

