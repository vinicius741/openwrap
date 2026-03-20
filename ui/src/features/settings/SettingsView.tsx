import { useEffect, useState } from 'react'

import { useAppStore } from '../../store/appStore'

export function SettingsView() {
  const settings = useAppStore((state) => state.settings)
  const detection = useAppStore((state) => state.detection)
  const saveSettings = useAppStore((state) => state.saveSettings)

  const [overridePath, setOverridePath] = useState('')
  const [verboseLogging, setVerboseLogging] = useState(false)

  useEffect(() => {
    setOverridePath(settings?.openvpn_path_override ?? '')
    setVerboseLogging(settings?.verbose_logging ?? false)
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

      <div className="settings-field">
        <label>
          <input
            type="checkbox"
            checked={verboseLogging}
            onChange={(event) => setVerboseLogging(event.target.checked)}
          />
          Verbose logging
        </label>
        <p className="settings-hint">Enable detailed logging for debugging connection issues</p>
      </div>

      <button
        className="action-button action-primary"
        onClick={() => void saveSettings(overridePath.trim() || null, verboseLogging)}
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

