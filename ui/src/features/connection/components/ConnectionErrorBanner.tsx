import type { UserFacingError } from '../../../types/ipc'
import { RuntimeSetup } from '../../settings/runtime-setup'

/**
 * Failures where OpenVPN itself did not run properly. An OpenVPN left broken
 * or outdated by a Homebrew update fails this way, so the banner offers to
 * update it.
 */
const OPENVPN_RUNTIME_FAILURES = new Set([
  'process_exit',
  'process_terminated',
  'process_not_found',
  'process_permission_denied',
  'openvpn_launch_failed',
  'openvpn_options_error',
])

interface ConnectionErrorBannerProps {
  error: UserFacingError | null
  logFilePath: string | null | undefined
  onShowLogs: () => void
  onRevealLog: () => void
}

export function ConnectionErrorBanner({ error, logFilePath, onShowLogs, onRevealLog }: ConnectionErrorBannerProps) {
  if (!error) {
    return null
  }

  return (
    <div className="error-banner">
      <div className="error-banner-copy">
        <strong>{error.title}</strong>
        <p>{error.message}</p>
        {error.details_safe ? (
          <p className="error-detail">
            <span>OpenVPN reported:</span> {error.details_safe}
          </p>
        ) : null}
        {error.suggested_fix ? <p>{error.suggested_fix}</p> : null}
        {error.code === 'openvpn_not_found' ? <RuntimeSetup /> : null}
        {OPENVPN_RUNTIME_FAILURES.has(error.code) ? <RuntimeSetup afterFailure /> : null}
        {logFilePath ? (
          <>
            <p className="error-path-label">Saved log file</p>
            <p className="log-file-path">{logFilePath}</p>
          </>
        ) : null}
      </div>
      <div className="error-banner-actions">
        <button className="action-button action-secondary action-small" onClick={onShowLogs} type="button">
          Show logs
        </button>
        {logFilePath ? (
          <button className="action-button action-secondary action-small" onClick={() => void onRevealLog()} type="button">
            Reveal log file
          </button>
        ) : null}
      </div>
    </div>
  )
}
