import { useEffect, useState } from 'react'

import { useAppStore } from '../../store/appStore'
import { THEMES, getStoredTheme, applyTheme, setStoredTheme, type AppTheme } from '../../lib/theme'

export function SettingsView() {
  const settings = useAppStore((state) => state.settings)
  const saveSettings = useAppStore((state) => state.saveSettings)
  const appInfo = useAppStore((state) => state.appInfo)
  const updateStatus = useAppStore((state) => state.updateStatus)
  const availableUpdate = useAppStore((state) => state.availableUpdate)
  const updateError = useAppStore((state) => state.updateError)
  const checkForUpdates = useAppStore((state) => state.checkForUpdates)
  const installUpdate = useAppStore((state) => state.installUpdate)

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

      <div className="section-heading">
        <div>
          <p className="eyebrow">Settings</p>
          <h3>Updates</h3>
        </div>
      </div>

      <div className="settings-field">
        <div className="settings-version-row">
          <span>Version {appInfo?.version ?? '…'}</span>
          <button
            className="action-button action-secondary action-small"
            disabled={updateStatus === 'checking' || updateStatus === 'installing'}
            onClick={() => void checkForUpdates(true)}
            type="button"
          >
            {updateStatus === 'checking' ? 'Checking…' : 'Check for updates'}
          </button>
          {availableUpdate && (updateStatus === 'available' || updateStatus === 'error') ? (
            <button
              className="action-button action-primary action-small"
              onClick={() => void installUpdate()}
              type="button"
            >
              Update to {availableUpdate.version}
            </button>
          ) : null}
        </div>
        {updateStatus === 'up_to_date' ? <p className="settings-hint">OpenWrap is up to date.</p> : null}
        {updateStatus === 'installing' ? <p className="settings-hint">Installing the update…</p> : null}
        {updateStatus === 'error' && updateError ? (
          <p className="settings-hint error-text">
            {updateError.title}: {updateError.message}
          </p>
        ) : null}
        <p className="settings-hint">
          OpenWrap checks GitHub for a new version every time it starts. Updates are verified with the
          project's signing key before they are installed.
        </p>
      </div>

    </div>
  )
}
