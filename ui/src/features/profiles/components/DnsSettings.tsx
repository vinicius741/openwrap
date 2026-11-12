import { useMemo, useState } from 'react'
import type { DnsPolicy, ProfileDetail } from '../../../types/ipc'
import {
  addDomain,
  findCoveringDomain,
  parseDomainInput,
  removeDomain,
  sortDomains,
} from '../model/dns-domains'

interface DnsSettingsProps {
  profile: ProfileDetail
  isUpdating: boolean
  /** The profile is connected and was started before the latest DNS change. */
  needsReconnect: boolean
  isReconnecting: boolean
  onPolicyChange: (policy: DnsPolicy) => Promise<void>
  onDomainsChange: (domains: string[]) => Promise<void>
  onReconnect: () => Promise<void>
}

const MODES: { policy: DnsPolicy; label: string; hint: string }[] = [
  {
    policy: 'FullOverride',
    label: 'All DNS through VPN',
    hint: "Every DNS lookup goes to the VPN's DNS server while connected.",
  },
  {
    policy: 'SplitDnsPreferred',
    label: 'Only listed domains',
    hint: "Only lookups for the domains below go to the VPN's DNS server. Everything else uses your normal DNS.",
  },
]

export function DnsSettings({
  profile,
  isUpdating,
  needsReconnect,
  isReconnecting,
  onPolicyChange,
  onDomainsChange,
  onReconnect,
}: DnsSettingsProps) {
  const policy = profile.profile.dns_policy
  const activeMode = MODES.find((mode) => mode.policy === policy)

  return (
    <div className="config-field">
      <span className="config-label" id="dns-mode-label">
        DNS
      </span>
      <div className="segmented" role="radiogroup" aria-labelledby="dns-mode-label">
        {MODES.map((mode) => (
          <button
            key={mode.policy}
            className="segmented-option"
            role="radio"
            aria-checked={policy === mode.policy}
            disabled={isUpdating}
            onClick={() => {
              if (policy !== mode.policy) void onPolicyChange(mode.policy)
            }}
            type="button"
          >
            {mode.label}
          </button>
        ))}
      </div>
      <p className="config-hint">
        {activeMode?.hint ?? 'OpenWrap leaves DNS untouched for this profile. Pick a mode to change that.'}
      </p>

      {policy === 'SplitDnsPreferred' && (
        <DomainList
          domains={profile.profile.dns_match_domains ?? []}
          isUpdating={isUpdating}
          onDomainsChange={onDomainsChange}
        />
      )}

      {needsReconnect && (
        <div className="dns-reconnect-notice" role="status">
          <p>DNS changes apply the next time you connect.</p>
          <button
            className="action-button action-secondary action-small"
            disabled={isReconnecting}
            onClick={() => void onReconnect()}
            type="button"
          >
            {isReconnecting ? 'Reconnecting…' : 'Reconnect now'}
          </button>
        </div>
      )}
    </div>
  )
}

function DomainList({
  domains,
  isUpdating,
  onDomainsChange,
}: {
  domains: string[]
  isUpdating: boolean
  onDomainsChange: (domains: string[]) => Promise<void>
}) {
  const [draft, setDraft] = useState('')
  const [choice, setChoice] = useState<string | null>(null)

  const listed = useMemo(() => sortDomains(domains), [domains])
  const parsed = useMemo(() => parseDomainInput(draft), [draft])
  const options = parsed.kind === 'ok' ? parsed.options : []
  // The broadest option is preselected so one paste covers future subdomains.
  const selected = choice !== null && options.includes(choice) ? choice : (options[0] ?? null)
  const coveredBy = parsed.kind === 'ok' ? findCoveringDomain(parsed.host, listed) : null
  const canAdd = selected !== null && coveredBy === null && !isUpdating

  const handleAdd = async () => {
    if (!canAdd || selected === null) return
    await onDomainsChange(addDomain(listed, selected))
    setDraft('')
    setChoice(null)
  }

  return (
    <div className="dns-domains">
      <form
        className="dns-domain-form"
        onSubmit={(event: React.FormEvent) => {
          event.preventDefault()
          void handleAdd()
        }}
      >
        <input
          aria-label="Link or domain to add"
          autoCapitalize="off"
          autoComplete="off"
          autoCorrect="off"
          placeholder="Paste a link or type a domain"
          spellCheck={false}
          value={draft}
          onChange={(event: React.ChangeEvent<HTMLInputElement>) => {
            setDraft(event.target.value)
            setChoice(null)
          }}
        />
        <button className="action-button action-primary" disabled={!canAdd} type="submit">
          Add
        </button>
      </form>

      {parsed.kind === 'error' && <p className="config-hint dns-domain-error">{parsed.message}</p>}

      {parsed.kind === 'ok' && coveredBy !== null && (
        <p className="config-hint">
          <code>{parsed.host}</code> is already covered by <code>{coveredBy}</code>.
        </p>
      )}

      {parsed.kind === 'ok' && coveredBy === null && (
        <div className="dns-options" role="radiogroup" aria-label="Domain to add">
          {options.map((option, index) => (
            <button
              key={option}
              className="dns-option"
              role="radio"
              aria-checked={option === selected}
              onClick={() => setChoice(option)}
              type="button"
            >
              <span className="dns-option-mark" aria-hidden="true" />
              <code>{option}</code>
              <span className="dns-option-hint">
                {index === 0 && options.length > 1
                  ? 'Covers the most: this domain and everything under it'
                  : index === options.length - 1 && options.length > 1
                    ? 'Only this address and anything under it'
                    : 'This domain and everything under it'}
              </span>
            </button>
          ))}
        </div>
      )}

      {listed.length === 0 ? (
        <p className="config-hint dns-domain-empty">
          No domains listed yet. Until you add one, the VPN&apos;s DNS is not used, so sites that only exist
          inside the VPN will not open.
        </p>
      ) : (
        <ul className="dns-domain-list" aria-label="Domains using VPN DNS">
          {listed.map((domain) => (
            <li key={domain}>
              <code>{domain}</code>
              <button
                className="icon-btn"
                aria-label={`Remove ${domain}`}
                disabled={isUpdating}
                onClick={() => void onDomainsChange(removeDomain(listed, domain))}
                title="Remove"
                type="button"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
