import { ImportProfileDialog } from '../features/profiles/ImportProfileDialog'
import { ProfileList } from '../features/profiles/ProfileList'

export function Sidebar() {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-title">
          <p className="eyebrow">OpenWrap</p>
          <h1>Profiles</h1>
        </div>
        <ImportProfileDialog />
      </div>
      <ProfileList />
    </aside>
  )
}

