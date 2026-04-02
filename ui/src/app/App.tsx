import { useEffect, useState } from 'react'

import { Sidebar } from '../components/Sidebar'
import { TopBar } from '../components/TopBar'
import { ProfileDetail } from '../features/profiles/ProfileDetail'
import { SettingsView } from '../features/settings/SettingsView'
import { useConnectionEvents } from '../features/connection/useConnectionEvents'
import { useAppStore } from '../store/appStore'

export function App() {
  const loadInitial = useAppStore((state) => state.loadInitial)
  const selectedProfileId = useAppStore((state) => state.selectedProfileId)
  const error = useAppStore((state) => state.error)
  const setError = useAppStore((state) => state.setError)
  const helperInstalling = useAppStore((state) => state.helperInstalling)
  const installHelperAction = useAppStore((state) => state.installHelperAction)

  const [isSettingsOpen, setIsSettingsOpen] = useState(false)

  useConnectionEvents()

  useEffect(() => {
    void loadInitial()
  }, [loadInitial])

  return (
    <div className="shell">
      <TopBar
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      <div className="shell-body">
        <aside className="sidebar-persistent">
          <Sidebar />
        </aside>

        <main className="content">
          {error ? (
            <div className="error-banner app-error-banner">
              <div>
                <strong>{error.title}</strong>
                <p>{error.message}</p>
                {error.suggested_fix ? <p>{error.suggested_fix}</p> : null}
              </div>
              <div className="error-banner-actions">
                {error.code === 'helper_not_installed' ? (
                  <button
                    className="action-button action-primary"
                    disabled={helperInstalling}
                    onClick={() => void installHelperAction()}
                    type="button"
                  >
                    {helperInstalling ? 'Installing\u2026' : 'Install helper'}
                  </button>
                ) : null}
                <button className="action-button action-secondary" onClick={() => setError(null)} type="button">
                  Dismiss
                </button>
              </div>
            </div>
          ) : null}
          <ProfileDetail key={selectedProfileId ?? 'empty'} />
        </main>
      </div>

      {isSettingsOpen && (
        <>
          <div className="drawer-backdrop" onClick={() => setIsSettingsOpen(false)} />
          <div className="drawer drawer-right">
            <div className="drawer-header">
              <h2>Settings</h2>
              <button className="icon-btn" onClick={() => setIsSettingsOpen(false)} aria-label="Close settings">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
            <div className="drawer-content">
              <SettingsView />
            </div>
          </div>
        </>
      )}
    </div>
  )
}
