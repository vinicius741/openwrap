import { useEffect, useMemo, useState } from 'react'
import type { ProfileDetail } from '../../../types/ipc'

interface GeneratedPasswordCardProps {
  profile: ProfileDetail
  isSaving: boolean
  isClearing: boolean
  onSave: (input: { username: string; pin: string; totpSecret: string }) => Promise<void>
  onClear: () => Promise<void>
}

export function GeneratedPasswordCard({
  profile,
  isSaving,
  isClearing,
  onSave,
  onClear,
}: GeneratedPasswordCardProps) {
  const [username, setUsername] = useState('')
  const [pin, setPin] = useState('')
  const [totpSecret, setTotpSecret] = useState('')

  useEffect(() => {
    setUsername('')
    setPin('')
    setTotpSecret('')
  }, [profile.profile.id])

  const isConfigured = profile.profile.credential_strategy === 'PinTotp'
  const pinIsValid = /^\d{4}$/.test(pin)
  const canSave = username.trim().length > 0 && pinIsValid && totpSecret.trim().length > 0
  const strategyLabel = useMemo(
    () => (isConfigured ? 'Generated password enabled' : 'Prompt for credentials'),
    [isConfigured],
  )

  if (profile.profile.credential_mode !== 'UserPass') {
    return null
  }

  return (
    <article className="detail-card generated-password-card">
      <div className="generated-password-header">
        <div>
          <h3>VPN password handling</h3>
          <p className="card-description">
            Save a local PIN + TOTP configuration for this profile so Connect can build the password automatically.
          </p>
        </div>
        <span className={`status-badge ${isConfigured ? 'status-connected' : ''}`}>
          {strategyLabel}
        </span>
      </div>

      <form
        className="generated-password-form"
        onSubmit={(event) => {
          event.preventDefault()
          void onSave({
            username: username.trim(),
            pin,
            totpSecret: totpSecret.trim(),
          })
        }}
      >
        <label>
          Username
          <input
            autoComplete="username"
            placeholder="VPN username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
          />
        </label>
        <label>
          4-digit PIN
          <input
            autoComplete="off"
            inputMode="numeric"
            maxLength={4}
            placeholder="1234"
            pattern="[0-9]{4}"
            value={pin}
            onChange={(event) => {
              const nextValue = event.target.value.replace(/\D/g, '').slice(0, 4)
              setPin(nextValue)
            }}
          />
        </label>
        <label>
          TOTP secret
          <input
            autoComplete="off"
            placeholder="Base32 secret"
            type="password"
            value={totpSecret}
            onChange={(event) => setTotpSecret(event.target.value)}
          />
        </label>

        <p className="card-description">
          The PIN and TOTP secret stay local. OpenWrap will combine them at connect time and write only the derived password to the runtime auth file.
        </p>

        <div className="form-actions">
          <button className="action-button action-primary" disabled={!canSave || isSaving} type="submit">
            {isSaving ? 'Saving...' : 'Save generated password'}
          </button>
          <button
            className="action-button action-secondary"
            disabled={!isConfigured || isClearing}
            onClick={() => void onClear()}
            type="button"
          >
            {isClearing ? 'Clearing...' : 'Clear saved config'}
          </button>
        </div>
      </form>
    </article>
  )
}
