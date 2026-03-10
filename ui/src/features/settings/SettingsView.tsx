import { useEffect, useState } from 'react'

import { useAppStore } from '../../store/appStore'

export function SettingsView() {
  const settings = useAppStore((state) => state.settings)
  const detection = useAppStore((state) => state.detection)
  const saveSettings = useAppStore((state) => state.saveSettings)

  const [overridePath, setOverridePath] = useState('')

  useEffect(() => {
    setOverridePath(settings?.openvpn_path_override ?? '')
  }, [settings])

  return (
    <div className="settings-view">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Settings</p>
          <h3>Runtime</h3>
        </div>
      </div>

      <div className="settings-field">
        <label>OpenVPN binary override</label>
        <input
          placeholder="/opt/homebrew/sbin/openvpn"
          value={overridePath}
          onChange={(event) => setOverridePath(event.target.value)}
        />
      </div>

      <button
        className="action-button action-primary"
        onClick={() => void saveSettings(overridePath.trim() || null)}
        type="button"
      >
        Save
      </button>

      <div className="settings-detail">
        <h4>Detected binaries</h4>
        <ul className="asset-list">
          {(detection?.discovered_paths ?? []).map((path) => (
            <li key={path}>
              <strong>{path === detection?.selected_path ? 'Selected' : 'Found'}</strong>
              <span>{path}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  )
}

