import { useEffect, useState } from 'react'

import { useAppStore } from '../../store/appStore'
import { THEMES, getStoredTheme, applyTheme, setStoredTheme, type AppTheme } from '../../lib/theme'

export function SettingsView() {
  const settings = useAppStore((state) => state.settings)
  const saveSettings = useAppStore((state) => state.saveSettings)

  const [verboseLogging, setVerboseLogging] = useState(false)
  const [activeTheme, setActiveTheme] = useState<AppTheme>(() => getStoredTheme())

  useEffect(() => {
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
              <span className="theme-personality">{theme.personality}</span>
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

      <div className="settings-detail">
        <h4>OpenVPN community runtime</h4>
        <p className="settings-hint">
          OpenWrap uses a locally installed privileged helper to launch OpenVPN and reconcile DNS.
          Install the helper once using the command documented in the README.
        </p>
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
        onClick={() => void saveSettings(verboseLogging)}
        type="button"
      >
        Save
      </button>

    </div>
  )
}
