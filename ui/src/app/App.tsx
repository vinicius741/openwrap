import { useEffect } from 'react'

import { Sidebar } from '../components/Sidebar'
import { ProfileDetail } from '../features/profiles/ProfileDetail'
import { SettingsView } from '../features/settings/SettingsView'
import { useConnectionEvents } from '../features/connection/useConnectionEvents'
import { useAppStore } from '../store/appStore'

export function App() {
  const loadInitial = useAppStore((state) => state.loadInitial)
  const selectedProfileId = useAppStore((state) => state.selectedProfileId)

  useConnectionEvents()

  useEffect(() => {
    void loadInitial()
  }, [loadInitial])

  return (
    <div className="shell">
      <Sidebar />
      <main className="content">
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

