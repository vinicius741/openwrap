import { useState } from 'react'

import { StatusBadge } from '../../components/StatusBadge'
import { useAppStore } from '../../store/appStore'

export function ConnectionPanel() {
  const connection = useAppStore((state) => state.connection)
  const selectedProfile = useAppStore((state) => state.selectedProfile)
  const prompt = useAppStore((state) => state.pendingCredentialPrompt)
  const connectSelected = useAppStore((state) => state.connectSelected)
  const disconnect = useAppStore((state) => state.disconnect)
  const submit = useAppStore((state) => state.submitCredentials)

  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [remember, setRemember] = useState(true)

  const isConnected =
    connection?.state === 'connected' ||
    connection?.state === 'connecting' ||
    connection?.state === 'reconnecting' ||
    connection?.state === 'awaiting_credentials'

  return (
    <section className="connection-panel">
      <div className="connection-summary">
        <div>
          <p className="eyebrow">Connection</p>
          <h3>{selectedProfile?.profile.remote_summary || 'No remote'}</h3>
        </div>
        <StatusBadge state={connection?.state} />
      </div>

      <div className="button-row">
        <button className="action-button action-primary" disabled={isConnected} onClick={() => void connectSelected()} type="button">
          Connect
        </button>
        <button className="action-button action-secondary" disabled={!isConnected} onClick={() => void disconnect()} type="button">
          Disconnect
        </button>
      </div>

      <div className="connection-metadata">
        <div>
          <span>PID</span>
          <strong>{connection?.pid ?? 'Not started'}</strong>
        </div>
        <div>
          <span>DNS mode</span>
          <strong>{connection?.dns_observation.effective_mode ?? 'ObserveOnly'}</strong>
        </div>
        <div>
          <span>Saved credentials</span>
          <strong>{selectedProfile?.profile.has_saved_credentials ? 'Yes' : 'No'}</strong>
        </div>
      </div>

      {connection?.dns_observation.warnings.length ? (
        <div className="dns-observation">
          <strong>DNS warnings</strong>
          <p>{connection.dns_observation.warnings.join(' ')}</p>
        </div>
      ) : null}

      {connection?.last_error ? (
        <div className="error-banner">
          <strong>{connection.last_error.title}</strong>
          <p>{connection.last_error.message}</p>
          {connection.last_error.suggested_fix ? <p>{connection.last_error.suggested_fix}</p> : null}
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
            Save in Keychain
          </label>
          <button className="action-button action-primary" type="submit">
            Continue
          </button>
        </form>
      ) : null}
    </section>
  )
}
