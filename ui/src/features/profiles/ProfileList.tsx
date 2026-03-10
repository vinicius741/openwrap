import { useAppStore } from '../../store/appStore'

export function ProfileList() {
  const profiles = useAppStore((state) => state.profiles)
  const selectedProfileId = useAppStore((state) => state.selectedProfileId)
  const selectProfile = useAppStore((state) => state.selectProfile)

  if (!profiles.length) {
    return <div className="sidebar-empty">Import a `.ovpn` file to get started.</div>
  }

  return (
    <div className="profile-list">
      {profiles.map((profile) => (
        <button
          key={profile.id}
          className={`profile-row ${profile.id === selectedProfileId ? 'is-selected' : ''}`}
          onClick={() => void selectProfile(profile.id)}
          type="button"
        >
          <span className="profile-name">{profile.name}</span>
          <span className="profile-remote">{profile.remote_summary || 'No remote summary'}</span>
          <span className={`profile-validation validation-${profile.validation_status.toLowerCase()}`}>
            {profile.validation_status.toLowerCase()}
          </span>
        </button>
      ))}
    </div>
  )
}

