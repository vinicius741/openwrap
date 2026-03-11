import { useState, useEffect, useCallback } from 'react'
import { EmptyState } from '../../components/EmptyState'
import { ConnectionPanel } from '../connection/ConnectionPanel'
import { LogPane } from '../logs/LogPane'
import { useAppStore } from '../../store/appStore'

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

export function ProfileDetail() {
  const profile = useAppStore((state) => state.selectedProfile)
  const deleteProfile = useAppStore((state) => state.deleteProfile)
  const [showDeleteModal, setShowDeleteModal] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)

  const closeModal = useCallback(() => {
    setShowDeleteModal(false)
    setDeleteError(null)
  }, [])

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && showDeleteModal) {
        closeModal()
      }
    }
    window.addEventListener('keydown', handleEscape)
    return () => window.removeEventListener('keydown', handleEscape)
  }, [showDeleteModal, closeModal])

  if (!profile) {
    return <EmptyState title="No profile selected" detail="Import a profile and select it from the sidebar." />
  }

  const handleDelete = async () => {
    setIsDeleting(true)
    setDeleteError(null)
    try {
      await deleteProfile(profile.profile.id)
      closeModal()
    } catch (error) {
      setDeleteError(error instanceof Error ? error.message : 'Failed to delete profile')
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="profile-detail">
      <header className="profile-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div>
          <p className="eyebrow">Managed profile</p>
          <h2>{profile.profile.name}</h2>
          <p className="profile-subtitle">{profile.profile.remote_summary || 'Remote summary unavailable'}</p>
        </div>
        <button
          className="icon-btn delete-btn"
          onClick={() => setShowDeleteModal(true)}
          type="button"
          title="Delete profile"
        >
          <TrashIcon />
        </button>
      </header>

      {showDeleteModal && (
        <div className="modal-backdrop" onClick={closeModal}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="modal-content">
              <h3>Delete Profile</h3>
              <p style={{ color: 'var(--text-secondary)', marginTop: '8px' }}>
                Are you sure you want to delete "{profile.profile.name}"? This action cannot be undone.
              </p>
              {deleteError && (
                <p className="error-text" style={{ marginTop: '12px' }}>
                  {deleteError}
                </p>
              )}
            </div>
            <div className="modal-actions" style={{ marginTop: '24px' }}>
              <button
                className="action-button action-secondary"
                onClick={closeModal}
                type="button"
                disabled={isDeleting}
                autoFocus
              >
                Cancel
              </button>
              <button
                className="action-button action-danger"
                onClick={handleDelete}
                type="button"
                disabled={isDeleting}
              >
                {isDeleting ? 'Deleting...' : 'Delete'}
              </button>
            </div>
          </div>
        </div>
      )}
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

