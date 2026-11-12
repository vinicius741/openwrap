import { useEffect, useState } from 'react'

import { Sidebar } from '../components/Sidebar'
import { TopBar } from '../components/TopBar'
import { ProfileDetail } from '../features/profiles/ProfileDetail'
import { SettingsView } from '../features/settings/SettingsView'
import { OpenVpnBrokenBanner, RuntimeSetup } from '../features/settings/runtime-setup'
import { useConnectionEvents } from '../features/connection/useConnectionEvents'
import { HelperOutdatedBanner, UpdateBanner } from '../features/updates/UpdateBanner'
import { useUpdateEvents } from '../features/updates/useUpdateEvents'
import { WhatsNewDialog } from '../features/updates/WhatsNewDialog'
import { useAppStore } from '../store/appStore'

export function App() {
  const loadInitial = useAppStore((state) => state.loadInitial)
  const selectedProfileId = useAppStore((state) => state.selectedProfileId)
  const error = useAppStore((state) => state.error)
  const setError = useAppStore((state) => state.setError)
  const checkRuntimeDependencies = useAppStore((state) => state.checkRuntimeDependencies)

  const [isSettingsOpen, setIsSettingsOpen] = useState(false)

  useConnectionEvents()
  useUpdateEvents()

  useEffect(() => {
    void loadInitial()
  }, [loadInitial])

  // Notice at launch when OpenVPN can no longer start (for example after a
  // Homebrew update), instead of only when a connection fails.
  useEffect(() => {
    void checkRuntimeDependencies()
  }, [checkRuntimeDependencies])

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
          <UpdateBanner />
          <HelperOutdatedBanner />
          <OpenVpnBrokenBanner />
          {error ? (
            <div className="error-banner app-error-banner">
              <div>
                <strong>{error.title}</strong>
                <p>{error.message}</p>
                {error.suggested_fix ? <p>{error.suggested_fix}</p> : null}
                {error.code === 'openvpn_not_found' ? <RuntimeSetup /> : null}
              </div>
              <div className="error-banner-actions">
                <button className="action-button action-secondary" onClick={() => setError(null)} type="button">
                  Dismiss
                </button>
              </div>
            </div>
          ) : null}
          <ProfileDetail key={selectedProfileId ?? 'empty'} />
        </main>
      </div>

      <WhatsNewDialog />

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
