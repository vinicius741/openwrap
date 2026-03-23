type DnsPolicy = 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly'

interface DnsPolicyCardProps {
  currentPolicy: DnsPolicy
  isUpdating: boolean
  onPolicyChange: (policy: DnsPolicy) => Promise<void>
}

function isDnsPolicy(value: string): value is DnsPolicy {
  return (
    value === 'SplitDnsPreferred' ||
    value === 'FullOverride' ||
    value === 'ObserveOnly'
  )
}

export function DnsPolicyCard({ currentPolicy, isUpdating, onPolicyChange }: DnsPolicyCardProps) {
  const handleChange = (event: React.ChangeEvent<HTMLSelectElement>) => {
    if (isDnsPolicy(event.target.value)) {
      void onPolicyChange(event.target.value)
    }
  }

  return (
    <article className="detail-card">
      <h3>DNS policy</h3>
      <label style={{ display: 'grid', gap: '8px' }}>
        <span style={{ color: 'var(--text-secondary)', fontSize: '13px' }}>
          Choose how this profile applies VPN DNS on macOS.
        </span>
        <select
          value={currentPolicy}
          disabled={isUpdating}
          onChange={handleChange}
        >
          <option value="SplitDnsPreferred">Split DNS preferred</option>
          <option value="FullOverride">Full override</option>
          <option value="ObserveOnly">Observe only</option>
        </select>
        <span style={{ color: 'var(--text-secondary)', fontSize: '13px' }}>
          Full override replaces system DNS while connected and restores the previous DNS settings when you disconnect.
        </span>
      </label>
    </article>
  )
}
