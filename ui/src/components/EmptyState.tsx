export function EmptyState({
  title,
  detail,
}: {
  title: string
  detail: string
}) {
  return (
    <div className="empty-state">
      <h2>{title}</h2>
      <p>{detail}</p>
    </div>
  )
}

