import { useEffect, useCallback } from 'react'

interface DeleteProfileDialogProps {
  isOpen: boolean
  profileName: string
  isDeleting: boolean
  error: string | null
  onClose: () => void
  onConfirm: () => void
}

export function DeleteProfileDialog({
  isOpen,
  profileName,
  isDeleting,
  error,
  onClose,
  onConfirm,
}: DeleteProfileDialogProps) {
  const handleEscape = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        onClose()
      }
    },
    [isOpen, onClose],
  )

  useEffect(() => {
    window.addEventListener('keydown', handleEscape)
    return () => window.removeEventListener('keydown', handleEscape)
  }, [handleEscape])

  if (!isOpen) {
    return null
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-content">
          <h3>Delete Profile</h3>
          <p style={{ color: 'var(--text-secondary)', marginTop: '8px' }}>
            Are you sure you want to delete "{profileName}"? This action cannot be undone.
          </p>
          {error && (
            <p className="error-text" style={{ marginTop: '12px' }}>
              {error}
            </p>
          )}
        </div>
        <div className="modal-actions" style={{ marginTop: '24px' }}>
          <button
            className="action-button action-secondary"
            onClick={onClose}
            type="button"
            disabled={isDeleting}
            autoFocus
          >
            Cancel
          </button>
          <button
            className="action-button action-danger"
            onClick={onConfirm}
            type="button"
            disabled={isDeleting}
          >
            {isDeleting ? 'Deleting...' : 'Delete'}
          </button>
        </div>
      </div>
    </div>
  )
}
