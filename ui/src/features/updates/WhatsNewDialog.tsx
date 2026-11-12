import { useCallback, useEffect, useState } from 'react'

import { useAppStore } from '../../store/appStore'
import { WHATS_NEW, hasUnseenWhatsNew, markWhatsNewSeen } from './whats-new'

/** Shows the current release notes once, the first time the app opens after an update. */
export function WhatsNewDialog() {
  const version = useAppStore((state) => state.appInfo?.version ?? null)
  const [isOpen, setIsOpen] = useState(hasUnseenWhatsNew)

  const close = useCallback(() => {
    markWhatsNewSeen()
    setIsOpen(false)
  }, [])

  useEffect(() => {
    if (!isOpen) return
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close()
    }
    window.addEventListener('keydown', handleEscape)
    return () => window.removeEventListener('keydown', handleEscape)
  }, [isOpen, close])

  if (!isOpen) {
    return null
  }

  return (
    <div className="modal-backdrop" onClick={close}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="whats-new-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="modal-content">
          <p className="eyebrow">{version ? `What's new in ${version}` : "What's new"}</p>
          <h3 id="whats-new-title">{WHATS_NEW.headline}</h3>
          <ul className="whats-new-points">
            {WHATS_NEW.points.map((point) => (
              <li key={point.title}>
                <strong>{point.title}</strong>
                <p>{point.detail}</p>
              </li>
            ))}
          </ul>
        </div>
        <div className="modal-actions whats-new-actions">
          <button className="action-button action-primary" onClick={close} type="button" autoFocus>
            Got it
          </button>
        </div>
      </div>
    </div>
  )
}
