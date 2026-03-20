import { useEffect, useState } from 'react'

import { StatusBadge } from '../../components/StatusBadge'
import { normalizeCommandError } from '../../lib/tauri'
import { revealConnectionLogInFinder } from '../logs/api'
import { useAppStore } from '../../store/appStore'
import type { DnsObservation } from '../../types/ipc'

function getDnsStatusMessage(observation: DnsObservation | undefined): string | null {
  if (!observation) {
    return null
  }

  if (observation.restore_status === 'restore_failed') {
    return 'DNS restore failed; OpenWrap will retry reconciliation on next launch or when you reconnect.'
  }

  if (observation.restore_status === 'pending_reconcile') {
    return 'DNS restore is pending reconciliation; OpenWrap will retry on the next launch or reconnect.'
  }

  if (observation.auto_promoted_policy === 'FullOverride') {
    return 'OpenWrap auto-promoted this connection to full DNS override because the VPN did not provide split-DNS domains.'
  }

  if (observation.effective_mode === 'ScopedResolvers') {
    return 'Split DNS succeeded. OpenWrap is using VPN DNS only for configured domains.'
  }

  if (observation.effective_mode === 'GlobalOverride') {
    return 'Full DNS override is active. OpenWrap is routing all system DNS through the VPN.'
  }

  return null
}

export function ConnectionPanel() {
  const connection = useAppStore((state) => state.connection)
  const selectedProfile = useAppStore((state) => state.selectedProfile)
  const prompt = useAppStore((state) => state.pendingCredentialPrompt)
  const connectSelected = useAppStore((state) => state.connectSelected)
  const disconnect = useAppStore((state) => state.disconnect)
  const submit = useAppStore((state) => state.submitCredentials)
  const setError = useAppStore((state) => state.setError)

  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [remember, setRemember] = useState(true)

  useEffect(() => {
    if (!prompt) {
      return
    }

    setUsername(prompt.saved_username ?? '')
    setPassword('')
    setRemember(true)
  }, [prompt])

  const isConnected =
    connection?.state === 'connected' ||
    connection?.state === 'connecting' ||
    connection?.state === 'reconnecting' ||
    connection?.state === 'awaiting_credentials'
  const dnsStatusMessage = getDnsStatusMessage(connection?.dns_observation)

  const handleShowLogs = () => {
    document.getElementById('connection-logs')?.scrollIntoView({
      behavior: 'smooth',
      block: 'start',
    })
  }

  const handleRevealLog = async () => {
    try {
      await revealConnectionLogInFinder()
    } catch (error) {
      setError(normalizeCommandError(error))
    }
  }

  return (
    <section className="connection-panel">
      <div className="connection-summary">
        <div>
          <p className="eyebrow">Connection</p>
          <h3>{selectedProfile?.profile.remote_summary || 'No remote'}</h3>
        </div>
        <StatusBadge state={connection?.state} />
      </div>

      <div className="connection-controls">
        <div className="button-group">
          <button className="action-button action-primary" disabled={isConnected} onClick={() => void connectSelected()} type="button">
            Connect
          </button>
          <button className="action-button action-secondary" disabled={!isConnected} onClick={() => void disconnect()} type="button">
            Disconnect
          </button>
        </div>

        <div className="connection-metadata">
          <div className="metadata-item">
            <span className="metadata-label">PID</span>
            <strong className="metadata-value">{connection?.pid ?? 'Not started'}</strong>
          </div>
          <div className="metadata-item">
            <span className="metadata-label">DNS mode</span>
            <strong className="metadata-value">{connection?.dns_observation.effective_mode ?? 'ObserveOnly'}</strong>
          </div>
          <div className="metadata-item">
            <span className="metadata-label">Saved username</span>
            <strong className="metadata-value">{selectedProfile?.profile.has_saved_credentials ? 'Yes' : 'No'}</strong>
          </div>
        </div>
      </div>

      {dnsStatusMessage ? (
        <div className="dns-observation">
          <strong>DNS status</strong>
          <p>{dnsStatusMessage}</p>
        </div>
      ) : null}

      {connection?.last_error ? (
        <div className="error-banner">
          <div className="error-banner-copy">
            <strong>{connection.last_error.title}</strong>
            <p>{connection.last_error.message}</p>
            {connection.last_error.details_safe ? (
              <p className="error-detail">
                <span>OpenVPN reported:</span> {connection.last_error.details_safe}
              </p>
            ) : null}
            {connection.last_error.suggested_fix ? <p>{connection.last_error.suggested_fix}</p> : null}
            {connection.log_file_path ? (
              <>
                <p className="error-path-label">Saved log file</p>
                <p className="log-file-path">{connection.log_file_path}</p>
              </>
            ) : null}
          </div>
          <div className="error-banner-actions">
            <button className="action-button action-secondary action-small" onClick={handleShowLogs} type="button">
              Show logs
            </button>
            {connection.log_file_path ? (
              <button
                className="action-button action-secondary action-small"
                onClick={() => void handleRevealLog()}
                type="button"
              >
                Reveal log file
              </button>
            ) : null}
          </div>
        </div>
      ) : null}

      {prompt ? (
        <form
          className="credential-form"
          onSubmit={(event) => {
            event.preventDefault()
            void submit({
              username,
              password,
              rememberInKeychain: remember,
            })
          }}
        >
          <h4>Credentials required</h4>
          <label>
            Username
            <input value={username} onChange={(event) => setUsername(event.target.value)} />
          </label>
          <label>
            Password
            <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} />
          </label>
          <label className="checkbox-row">
            <input checked={remember} onChange={(event) => setRemember(event.target.checked)} type="checkbox" />
            Remember username
          </label>
          <button className="action-button action-primary" type="submit">
            Continue
          </button>
        </form>
      ) : null}
    </section>
  )
}
