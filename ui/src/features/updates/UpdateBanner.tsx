import { useState } from 'react'

import { useAppStore } from '../../store/appStore'
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
  const [copied, setCopied] = useState(false)
  const [dismissed, setDismissed] = useState(false)

  if (!appInfo?.helper_install_command || dismissed) return null
  if (appInfo.helper_status !== 'outdated' && appInfo.helper_status !== 'missing') return null

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(appInfo.helper_install_command ?? '')
      setCopied(true)
    } catch {
      setCopied(false)
    }
  }

  return (
    <div className="update-banner">
      <div className="error-banner-copy">
        <strong>
          {appInfo.helper_status === 'missing'
            ? 'The OpenWrap helper is not installed'
            : 'The OpenWrap helper needs to be reinstalled'}
        </strong>
        <p>
          {appInfo.helper_status === 'missing'
            ? 'OpenWrap needs its privileged helper to start OpenVPN.'
            : 'This version of OpenWrap ships a newer helper.'}{' '}
          Run this command in Terminal once (it asks for your Mac password), then reopen OpenWrap:
        </p>
        <code className="update-command">{appInfo.helper_install_command}</code>
      </div>
      <div className="error-banner-actions">
        <button className="action-button action-primary" onClick={() => void copy()} type="button">
          {copied ? 'Copied' : 'Copy command'}
        </button>
        <button className="action-button action-secondary" onClick={() => setDismissed(true)} type="button">
          Dismiss
        </button>
      </div>
    </div>
  )
}
