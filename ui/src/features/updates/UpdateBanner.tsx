import { useState } from 'react'

import { normalizeCommandError } from '../../lib/tauri'
import { useAppStore } from '../../store/appStore'
import type { UserFacingError } from '../../types/ipc'
import { formatUpdateProgress } from './format'

export function UpdateBanner() {
  const availableUpdate = useAppStore((state) => state.availableUpdate)
  const updateStatus = useAppStore((state) => state.updateStatus)
  const updateProgress = useAppStore((state) => state.updateProgress)
  const updateError = useAppStore((state) => state.updateError)
  const dismissed = useAppStore((state) => state.updateBannerDismissed)
  const connectionState = useAppStore((state) => state.connection?.state ?? 'idle')
  const installUpdate = useAppStore((state) => state.installUpdate)
  const dismissUpdateBanner = useAppStore((state) => state.dismissUpdateBanner)

  if (!availableUpdate || dismissed) return null
  if (updateStatus !== 'available' && updateStatus !== 'installing' && updateStatus !== 'error') return null

  const installing = updateStatus === 'installing'
  const downloadDone =
    updateProgress?.total != null && updateProgress.downloaded >= updateProgress.total
  const vpnActive = connectionState !== 'idle' && connectionState !== 'error'

  return (
    <div className="update-banner">
      <div className="error-banner-copy">
        <strong>OpenWrap {availableUpdate.version} is available</strong>
        <p>
          You have version {availableUpdate.current_version}.
          {vpnActive ? ' Updating restarts OpenWrap and disconnects the VPN.' : ' Updating restarts OpenWrap.'}
        </p>
        {availableUpdate.notes ? <p className="update-notes">{availableUpdate.notes}</p> : null}
        {installing ? (
          <p className="update-progress">
            {downloadDone
              ? 'Installing… macOS may ask for your password.'
              : formatUpdateProgress(updateProgress)}
          </p>
        ) : null}
        {updateStatus === 'error' && updateError ? (
          <p className="error-text">
            {updateError.title}: {updateError.message}
          </p>
        ) : null}
      </div>
      <div className="error-banner-actions">
        <button
          className="action-button action-primary"
          disabled={installing}
          onClick={() => void installUpdate()}
          type="button"
        >
          {installing ? 'Updating…' : 'Update and restart'}
        </button>
        <button
          className="action-button action-secondary"
          disabled={installing}
          onClick={dismissUpdateBanner}
          type="button"
        >
          Later
        </button>
      </div>
    </div>
  )
}

export function HelperOutdatedBanner() {
  const appInfo = useAppStore((state) => state.appInfo)
  const installHelper = useAppStore((state) => state.installHelper)
  const [installing, setInstalling] = useState(false)
  const [error, setError] = useState<UserFacingError | null>(null)
  const [dismissed, setDismissed] = useState(false)

  if (!appInfo?.helper_install_available || dismissed) return null
  if (appInfo.helper_status !== 'outdated' && appInfo.helper_status !== 'missing') return null

  const install = async () => {
    if (installing) return
    setInstalling(true)
    setError(null)
    try {
      await installHelper()
    } catch (error) {
      setError(normalizeCommandError(error))
    } finally {
      setInstalling(false)
    }
  }

  return (
    <div className="update-banner">
      <div className="error-banner-copy">
        <strong>
          {appInfo.helper_status === 'missing'
            ? 'The OpenWrap helper is not installed'
            : 'The OpenWrap helper needs an update'}
        </strong>
        <p>
          {appInfo.helper_status === 'missing'
            ? 'OpenWrap needs its privileged helper to start OpenVPN.'
            : 'This version of OpenWrap needs to update or repair its helper.'}{' '}
          {installing
            ? 'Waiting for macOS authorization. Approve the installation in the system dialog.'
            : 'macOS will ask for an administrator password to install it. You can connect as soon as it finishes.'}
        </p>
        {error ? <p className="error-text" role="alert">{error.message}</p> : null}
      </div>
      <div className="error-banner-actions">
        <button className="action-button action-primary" disabled={installing} onClick={() => void install()} type="button">
          {installing ? 'Installing…' : appInfo.helper_status === 'missing' ? 'Install helper' : 'Update helper'}
        </button>
        <button className="action-button action-secondary" disabled={installing} onClick={() => setDismissed(true)} type="button">
          Later
        </button>
      </div>
    </div>
  )
}
