import { useEffect } from 'react'

import { useAppStore } from '../../store/appStore'

export function RuntimeSetup() {
  const dependencies = useAppStore((state) => state.runtimeDependencies)
  const status = useAppStore((state) => state.runtimeStatus)
  const error = useAppStore((state) => state.runtimeError)
  const check = useAppStore((state) => state.checkRuntimeDependencies)
  const install = useAppStore((state) => state.installRuntimeDependencies)

  useEffect(() => { void check() }, [check])

  const busy = status === 'installing' || status === 'checking'

  return (
    <div className="runtime-setup" aria-busy={busy}>
      <div role="status" aria-live="polite">
        {status === 'ready' ? (
          <>
            <p>OpenVPN is installed.</p>
            <p className="helper-path">{dependencies?.openvpn_path}</p>
          </>
        ) : (
          <p className="settings-hint">
            {status === 'installing'
              ? 'Complete setup in Terminal. Enter your Mac password when asked and keep the window open until installation finishes.'
              : 'Install OpenVPN and, if needed, Homebrew. Setup opens Terminal and asks for your Mac administrator password. It may take several minutes.'}
          </p>
        )}
      </div>
      {error ? <p className="error-text" role="alert">{error.message} {error.suggested_fix}</p> : null}
      <div className="error-banner-actions">
        {status !== 'ready' ? (
          <button
            className="action-button action-primary action-small"
            disabled={busy}
            onClick={() => void install()}
            type="button"
          >
            {status === 'installing' ? 'Installing…' : status === 'checking' ? 'Checking…' : 'Install OpenVPN'}
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
