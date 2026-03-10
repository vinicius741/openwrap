import { useEffect } from 'react'

import { Sidebar } from '../components/Sidebar'
import { ProfileDetail } from '../features/profiles/ProfileDetail'
import { SettingsView } from '../features/settings/SettingsView'
import { useConnectionEvents } from '../features/connection/useConnectionEvents'
import { useAppStore } from '../store/appStore'

export function App() {
  const loadInitial = useAppStore((state) => state.loadInitial)
  const selectedProfileId = useAppStore((state) => state.selectedProfileId)
  const error = useAppStore((state) => state.error)
  const setError = useAppStore((state) => state.setError)

  useConnectionEvents()

  useEffect(() => {
    void loadInitial()
  }, [loadInitial])

  return (
    <div className="shell">
      <Sidebar />
      <main className="content">
        {error ? (
          <div className="error-banner app-error-banner">
            <div>
              <strong>{error.title}</strong>
              <p>{error.message}</p>
              {error.suggested_fix ? <p>{error.suggested_fix}</p> : null}
            </div>
            <button className="action-button action-secondary" onClick={() => setError(null)} type="button">
              Dismiss
            </button>
          </div>
        ) : null}
        <section className="panel panel-main">
          <ProfileDetail key={selectedProfileId ?? 'empty'} />
        </section>
        <aside className="panel panel-settings">
          <SettingsView />
        </aside>
      </main>
    </div>
  )
}
