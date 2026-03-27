import { useEffect, useState } from 'react'
import type { ProfileDetail } from '../../../types/ipc'

const PIN_MASK = '••••'
const TOTP_MASK = '••••••••'

interface ConfigSectionProps {
  profile: ProfileDetail
  isSavingPassword: boolean
  isClearingPassword: boolean
  isUpdatingDns: boolean
  onDnsPolicyChange: (policy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly') => Promise<void>
  onGeneratedPasswordSave: (input: { username: string; pin: string; totpSecret: string }) => Promise<void>
  onGeneratedPasswordClear: () => Promise<void>
}

function isDnsPolicy(value: string): value is 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly' {
  return (
    value === 'SplitDnsPreferred' ||
    value === 'FullOverride' ||
    value === 'ObserveOnly'
  )
}

export function ConfigSection({
  profile,
  isSavingPassword,
  isClearingPassword,
  isUpdatingDns,
  onDnsPolicyChange,
  onGeneratedPasswordSave,
  onGeneratedPasswordClear,
}: ConfigSectionProps) {
  const currentPolicy = profile.profile.dns_policy
  const isPasswordConfigured = profile.profile.credential_strategy === 'PinTotp'
  const showsPassword = profile.profile.credential_mode === 'UserPass'

  return (
    <section className="config-section">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Configuration</p>
          <h3>Profile settings</h3>
        </div>
      </div>

      <div className="config-fields">
        <div className="config-field">
          <label>
            DNS policy
            <select
              value={currentPolicy}
              disabled={isUpdatingDns}
              onChange={(event) => {
                if (isDnsPolicy(event.target.value)) {
                  void onDnsPolicyChange(event.target.value)
                }
              }}
            >
              <option value="SplitDnsPreferred">Split DNS preferred</option>
              <option value="FullOverride">Full override</option>
              <option value="ObserveOnly">Observe only</option>
            </select>
          </label>
          <p className="config-hint">
            {currentPolicy === 'FullOverride'
              ? 'Replaces system DNS while connected, restores on disconnect.'
              : currentPolicy === 'SplitDnsPreferred'
                ? 'Routes VPN domain queries through the tunnel DNS, leaves the rest untouched.'
                : 'Watches DNS changes without modifying them.'}
          </p>
        </div>

        {showsPassword && (
          <PasswordSubForm
            profile={profile}
            isConfigured={isPasswordConfigured}
            isSaving={isSavingPassword}
            isClearing={isClearingPassword}
            onSave={onGeneratedPasswordSave}
            onClear={onGeneratedPasswordClear}
          />
        )}
      </div>
    </section>
  )
}

function PasswordSubForm({
  profile,
  isConfigured,
  isSaving,
  isClearing,
  onSave,
  onClear,
}: {
  profile: ProfileDetail
  isConfigured: boolean
  isSaving: boolean
  isClearing: boolean
  onSave: (input: { username: string; pin: string; totpSecret: string }) => Promise<void>
  onClear: () => Promise<void>
}) {
  const [username, setUsername] = useState('')
  const [pin, setPin] = useState('')
  const [totpSecret, setTotpSecret] = useState('')

  useEffect(() => {
    if (profile.has_saved_pin_totp) {
      setUsername(profile.saved_username ?? '')
      setPin(PIN_MASK)
      setTotpSecret(TOTP_MASK)
    } else {
      setUsername(profile.saved_username ?? '')
      setPin('')
      setTotpSecret('')
    }
  }, [profile])

  const isMasked = pin === PIN_MASK && totpSecret === TOTP_MASK
  const pinIsValid = pin === PIN_MASK || /^\d{4}$/.test(pin)
  const canSave = !isMasked && username.trim().length > 0 && pinIsValid && totpSecret.length > 0

  return (
    <div className="config-field">
      <label>
        VPN password handling
      </label>
      <div className="config-password-status">
        <span className={`status-badge ${isConfigured ? 'status-connected' : ''}`}>
          {isConfigured ? 'Generated password enabled' : 'Prompt for credentials'}
        </span>
      </div>
      <p className="config-hint">
        Save a PIN + TOTP configuration so Connect can build the password automatically.
      </p>
      <form
        className="config-password-form"
        onSubmit={(event: React.FormEvent) => {
          event.preventDefault()
          void onSave({
            username: username.trim(),
            pin,
            totpSecret: totpSecret.trim(),
          })
        }}
      >
        <div className="config-form-row">
          <label>
            Username
            <input
              autoComplete="username"
              placeholder="VPN username"
              value={username}
              onChange={(event: React.ChangeEvent<HTMLInputElement>) => setUsername(event.target.value)}
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
              onChange={(event: React.ChangeEvent<HTMLInputElement>) => {
                const nextValue = event.target.value.replace(/\D/g, '').slice(0, 4)
                setPin(nextValue)
              }}
              onFocus={() => {
                if (pin === PIN_MASK) setPin('')
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
              onChange={(event: React.ChangeEvent<HTMLInputElement>) => setTotpSecret(event.target.value)}
              onFocus={() => {
                if (totpSecret === TOTP_MASK) setTotpSecret('')
              }}
            />
          </label>
        </div>
        <div className="form-actions">
          <button className="action-button action-primary action-small" disabled={!canSave || isSaving} type="submit">
            {isSaving ? 'Saving...' : 'Save'}
          </button>
          <button
            className="action-button action-secondary action-small"
            disabled={!isConfigured || isClearing}
            onClick={() => void onClear()}
            type="button"
          >
            {isClearing ? 'Clearing...' : 'Clear'}
          </button>
        </div>
      </form>
    </div>
  )
}
