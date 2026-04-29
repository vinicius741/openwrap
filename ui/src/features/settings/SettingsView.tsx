import { useEffect, useState } from 'react'

import { useAppStore } from '../../store/appStore'
import { THEMES, getStoredTheme, applyTheme, setStoredTheme, type AppTheme } from '../../lib/theme'

export function SettingsView() {
  const settings = useAppStore((state) => state.settings)
  const detection = useAppStore((state) => state.detection)
  const saveSettings = useAppStore((state) => state.saveSettings)
  const helperStatus = useAppStore((state) => state.helperStatus)
  const helperInstalling = useAppStore((state) => state.helperInstalling)
  const installHelperAction = useAppStore((state) => state.installHelperAction)

  const [overridePath, setOverridePath] = useState('')
  const [verboseLogging, setVerboseLogging] = useState(false)
  const [activeTheme, setActiveTheme] = useState<AppTheme>(() => getStoredTheme())

  useEffect(() => {
    setOverridePath(settings?.openvpn_path_override ?? '')
    setVerboseLogging(settings?.verbose_logging ?? false)
  }, [settings])

  const handleThemeChange = (theme: AppTheme) => {
    setActiveTheme(theme)
    applyTheme(theme)
    setStoredTheme(theme)
  }

  return (
    <div className="settings-view">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Settings</p>
          <h3>Appearance</h3>
        </div>
      </div>

      <div className="settings-field">
        <label>Theme</label>
        <div className="theme-options">
          {THEMES.map((theme) => (
            <button
              key={theme.id}
              className={`theme-card ${activeTheme === theme.id ? 'is-active' : ''}`}
              onClick={() => handleThemeChange(theme.id)}
              type="button"
              aria-label={`Switch to ${theme.label} theme`}
            >
              <div className={`theme-preview theme-preview-${theme.id}`} />
              <span className="theme-label">{theme.label}</span>
              <span className="theme-font-name" data-font={theme.fontKey}>{theme.font}</span>
            </button>
          ))}
        </div>
      </div>

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
        <h4>Privileged helper</h4>
        <ul className="asset-list">
          <li>
            <strong>Status</strong>
            <span>
              {helperStatus?.installed ? (
                <span className="helper-status helper-status-ok">Installed</span>
              ) : (
                <span className="helper-status helper-status-missing">Not installed</span>
              )}
            </span>
          </li>
          <li>
            <strong>Path</strong>
            <span className="helper-path">{helperStatus?.helperPath ?? '—'}</span>
          </li>
          {helperStatus?.bundledHelperPath ? (
            <li>
              <strong>Bundled source</strong>
              <span className="helper-path">{helperStatus.bundledHelperPath}</span>
            </li>
          ) : null}
          {helperStatus?.reason ? (
            <li>
              <strong>Details</strong>
              <span className="helper-reason">{helperStatus.reason}</span>
            </li>
          ) : null}
        </ul>
        <button
          className="action-button action-primary"
          disabled={helperInstalling || helperStatus?.installed}
          onClick={() => void installHelperAction()}
          type="button"
        >
          {helperInstalling ? 'Installing\u2026' : helperStatus?.installed ? 'Installed' : 'Install helper'}
        </button>
        <p className="settings-hint">macOS will prompt for your password or Touch ID to authorize installation.</p>
      </div>

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
