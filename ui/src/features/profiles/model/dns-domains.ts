/**
 * Turns whatever the user pastes (a full URL, a host, a bare domain) into the
 * domains that can be listed for a profile's "only listed domains" DNS mode.
 *
 * A listed domain covers itself and every subdomain, so the broadest sensible
 * option is offered first. The backend re-validates every domain it stores;
 * the rules here mirror `normalize_match_domain` in openwrap-core.
 *
 * This file has no imports so the Node test runner can load it directly.
 */

export type DomainInput =
  | { kind: 'empty' }
  | { kind: 'error'; message: string }
  /** `options` runs from the broadest domain to the exact host pasted. */
  | { kind: 'ok'; host: string; options: string[] }

/**
 * Second-level labels that act as a public suffix under a country code
 * (`bmw.co.uk`, `bmw.com.br`), where the registrable domain has three labels.
 */
const COUNTRY_SECOND_LEVEL = new Set([
  'ac', 'co', 'com', 'edu', 'go', 'gob', 'gov', 'ind', 'mil', 'ne', 'net', 'nom', 'or', 'org',
])

const LABEL = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/

function extractHost(raw: string): string | null {
  const text = raw.trim()
  const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(text) ? text : `http://${text.replace(/^\/+/, '')}`
  try {
    return new URL(withScheme).hostname.toLowerCase()
  } catch {
    return null
  }
}

function isValidDomain(domain: string): boolean {
  if (domain.length === 0 || domain.length > 253) return false
  const labels = domain.split('.')
  if (!labels.every((label) => label.length <= 63 && LABEL.test(label))) return false
  // A numeric last label means an IP address, not a name.
  return !/^\d+$/.test(labels[labels.length - 1])
}

export function parseDomainInput(raw: string): DomainInput {
  if (raw.trim().length === 0) return { kind: 'empty' }

  const hostname = extractHost(raw)
  if (hostname === null || hostname.length === 0) {
    return { kind: 'error', message: 'That does not look like a link or a domain.' }
  }
  if (hostname.startsWith('[') || /^\d+(\.\d+){3}$/.test(hostname)) {
    return { kind: 'error', message: 'IP addresses do not use DNS, so there is nothing to add.' }
  }

  // Accept "*.example.com" and stray dots as "example.com".
  const host = hostname.replace(/^(\*\.)+/, '').replace(/^\.+|\.+$/g, '')
  if (!isValidDomain(host)) {
    return { kind: 'error', message: 'That does not look like a link or a domain.' }
  }

  const labels = host.split('.')
  const last = labels[labels.length - 1]
  const secondLast = labels[labels.length - 2]
  const suffixLabels =
    labels.length >= 3 && last.length === 2 && COUNTRY_SECOND_LEVEL.has(secondLast) ? 2 : 1
  const broadestLabels = Math.min(labels.length, suffixLabels + 1)

  const options: string[] = []
  for (let count = broadestLabels; count <= labels.length; count += 1) {
    options.push(labels.slice(labels.length - count).join('.'))
  }
  return { kind: 'ok', host, options }
}

/** True when `domain` is `parent` itself or one of its subdomains. */
export function isCoveredBy(domain: string, parent: string): boolean {
  return domain === parent || domain.endsWith(`.${parent}`)
}

/** The listed domain that already covers `domain`, if any. */
export function findCoveringDomain(domain: string, listed: string[]): string | null {
  return listed.find((entry) => isCoveredBy(domain, entry)) ?? null
}

/**
 * Adds `domain`, dropping listed entries it makes redundant, and returns the
 * list sorted so related names sit together (by parent domain first).
 */
export function addDomain(listed: string[], domain: string): string[] {
  if (findCoveringDomain(domain, listed)) return sortDomains(listed)
  return sortDomains([...listed.filter((entry) => !isCoveredBy(entry, domain)), domain])
}

export function removeDomain(listed: string[], domain: string): string[] {
  return listed.filter((entry) => entry !== domain)
}

export function sortDomains(domains: string[]): string[] {
  const key = (domain: string) => domain.split('.').reverse().join('.')
  return [...domains].sort((a, b) => key(a).localeCompare(key(b)))
}
