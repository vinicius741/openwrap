import { ImportProfileDialog } from '../features/profiles/ImportProfileDialog'
import { ProfileList } from '../features/profiles/ProfileList'

export function Sidebar() {
  return (
    <div className="sidebar-inner">
      <div className="sidebar-header">
        <div className="sidebar-title">
          <p className="eyebrow">OpenWrap</p>
          <h2>Profiles</h2>
        </div>
        <ImportProfileDialog />
      </div>
      <div className="sidebar-scroll">
        <ProfileList />
      </div>
    </div>
  )
}
