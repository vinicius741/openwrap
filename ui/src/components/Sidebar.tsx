import { ImportProfileDialog } from '../features/profiles/ImportProfileDialog'
import { ProfileList } from '../features/profiles/ProfileList'

interface SidebarProps {
  onClose: () => void
}

export function Sidebar({ onClose }: SidebarProps) {
  return (
    <div className="drawer drawer-left">
      <div className="drawer-header">
        <div className="sidebar-title">
          <p className="eyebrow">OpenWrap</p>
          <h2>Profiles</h2>
        </div>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <ImportProfileDialog />
          <button className="icon-btn" onClick={onClose} aria-label="Close profiles">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>
      <div className="drawer-content">
        <ProfileList />
      </div>
    </div>
  )
}
