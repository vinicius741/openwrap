function TrashIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18" />
      <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
      <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
      <line x1="10" y1="11" x2="10" y2="17" />
      <line x1="14" y1="11" x2="14" y2="17" />
    </svg>
  )
}

interface ProfileHeaderProps {
  name: string
  remoteSummary: string | null
  onDeleteClick: () => void
}

export function ProfileHeader({ name, remoteSummary, onDeleteClick }: ProfileHeaderProps) {
  return (
    <header className="profile-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
      <div>
        <p className="eyebrow">Managed profile</p>
        <h2>{name}</h2>
        <p className="profile-subtitle">{remoteSummary || 'Remote summary unavailable'}</p>
      </div>
      <button
        className="icon-btn delete-btn"
        onClick={onDeleteClick}
        type="button"
        title="Delete profile"
      >
        <TrashIcon />
      </button>
    </header>
  )
}
