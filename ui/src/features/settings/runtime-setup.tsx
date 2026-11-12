import { useEffect, useState } from 'react'

import { useAppStore } from '../../store/appStore'

interface RuntimeSetupProps {
  /** Shown under a failed connection, where updating OpenVPN is a likely fix. */
  afterFailure?: boolean
}

export function RuntimeSetup({ afterFailure = false }: RuntimeSetupProps) {
  const dependencies = useAppStore((state) => state.runtimeDependencies)
  const status = useAppStore((state) => state.runtimeStatus)
  const error = useAppStore((state) => state.runtimeError)
  const check = useAppStore((state) => state.checkRuntimeDependencies)
  const install = useAppStore((state) => state.installRuntimeDependencies)
  const repair = useAppStore((state) => state.repairRuntimeDependencies)

  useEffect(() => { void check() }, [check])

  const busy = status === 'installing' || status === 'repairing' || status === 'checking'
  // What is installed is read from the last detection, so it stays accurate
  // while a check runs or after a failed attempt.
  const installed = Boolean(dependencies?.openvpn_path)
  const broken = installed && Boolean(dependencies?.openvpn_problem)
  const canRepair = installed && Boolean(dependencies?.repair_available)

  return (
    <div className="runtime-setup" aria-busy={busy}>
      <div role="status" aria-live="polite">
        {status === 'repairing' ? (
          <p className="settings-hint">
            Homebrew is updating OpenVPN in Terminal. Keep the window open until it finishes. Homebrew may also
            update packages OpenVPN depends on.
          </p>
        ) : status === 'installing' ? (
          <p className="settings-hint">
            Complete setup in Terminal. Enter your Mac password when asked and keep the window open until
            installation finishes.
          </p>
        ) : broken ? (
          <>
            <p>OpenVPN is installed but cannot start. This usually happens after a Homebrew update.</p>
            <p className="helper-reason">{dependencies?.openvpn_problem}</p>
            {!canRepair ? (
              <p className="settings-hint">
                This OpenVPN was not installed with Homebrew, so OpenWrap cannot repair it. Reinstall it, then
                check again.
              </p>
            ) : null}
          </>
        ) : installed ? (
          <>
            <p>
              {dependencies?.openvpn_version
                ? `OpenVPN ${dependencies.openvpn_version} is installed.`
                : 'OpenVPN is installed.'}
            </p>
            <p className="helper-path">{dependencies?.openvpn_path}</p>
            {afterFailure && canRepair ? (
              <p className="settings-hint">
                If connections stopped working after a Homebrew update, updating OpenVPN usually fixes it.
              </p>
            ) : null}
          </>
        ) : (
          <p className="settings-hint">
            Install OpenVPN and, if needed, Homebrew. Setup opens Terminal and asks for your Mac administrator
            password. It may take several minutes.
          </p>
        )}
      </div>
      {error ? <p className="error-text" role="alert">{error.message} {error.suggested_fix}</p> : null}
      <div className="error-banner-actions">
        {!installed ? (
          <button
            className="action-button action-primary action-small"
            disabled={busy}
            onClick={() => void install()}
            type="button"
          >
            {status === 'installing' ? 'Installing…' : status === 'checking' ? 'Checking…' : 'Install OpenVPN'}
          </button>
        ) : canRepair ? (
          <button
            className={`action-button ${broken ? 'action-primary' : 'action-secondary'} action-small`}
            disabled={busy}
            onClick={() => void repair()}
            type="button"
          >
            {status === 'repairing' ? 'Updating…' : broken ? 'Repair OpenVPN' : 'Update OpenVPN'}
          </button>
        ) : null}
        <button
          className="action-button action-secondary action-small"
          disabled={busy}
          onClick={() => void check()}
          type="button"
        >
          Check again
        </button>
      </div>
    </div>
  )
}

/** App-wide notice when the installed OpenVPN cannot start, with a one-click repair. */
export function OpenVpnBrokenBanner() {
  const dependencies = useAppStore((state) => state.runtimeDependencies)
  const status = useAppStore((state) => state.runtimeStatus)
  const error = useAppStore((state) => state.runtimeError)
  const repair = useAppStore((state) => state.repairRuntimeDependencies)
  const [dismissed, setDismissed] = useState(false)

  const broken = Boolean(dependencies?.openvpn_path && dependencies.openvpn_problem)
  if (!broken || dismissed) return null

  const repairing = status === 'repairing'
  const canRepair = Boolean(dependencies?.repair_available)

  return (
    <div className="update-banner">
      <div className="error-banner-copy">
        <strong>OpenVPN stopped working</strong>
        <p>
          OpenVPN is installed but cannot start, which usually happens after a Homebrew update.{' '}
          {repairing
            ? 'Homebrew is updating it in Terminal. Keep the window open until it finishes.'
            : canRepair
              ? 'Repairing updates it with Homebrew in Terminal, which may also update related packages.'
              : 'It was not installed with Homebrew, so reinstall it to connect again.'}
        </p>
        <p className="helper-reason">{dependencies?.openvpn_problem}</p>
        {error ? <p className="error-text" role="alert">{error.message} {error.suggested_fix}</p> : null}
      </div>
      <div className="error-banner-actions">
        {canRepair ? (
          <button className="action-button action-primary" disabled={repairing} onClick={() => void repair()} type="button">
            {repairing ? 'Repairing…' : 'Repair OpenVPN'}
          </button>
        ) : null}
        <button className="action-button action-secondary" disabled={repairing} onClick={() => setDismissed(true)} type="button">
          Later
        </button>
      </div>
    </div>
  )
}
