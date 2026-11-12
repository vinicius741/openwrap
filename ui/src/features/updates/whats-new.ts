/**
 * The release notes shown once in the "What's new" window after an update.
 *
 * To announce a release, replace `WHATS_NEW` with the new notes and give it a
 * new `id`. Anyone who has not seen that id gets the window on next launch.
 * Keep it to a brief overview: a headline and a few short points.
 */

export interface WhatsNewNotes {
  /** Changes whenever there is something new to announce. */
  id: string
  headline: string
  points: { title: string; detail: string }[]
}

export const WHATS_NEW: WhatsNewNotes = {
  id: '2026-11-listed-dns-domains',
  headline: 'Choose which sites use the VPN for DNS',
  points: [
    {
      title: 'New DNS mode: only listed domains',
      detail:
        'A profile can now send DNS lookups to the VPN only for the domains you list. Everything else uses your normal DNS.',
    },
    {
      title: 'Paste a link to add a domain',
      detail:
        'Paste any address from your browser and OpenWrap picks out the domain. Changes apply when you reconnect.',
    },
    {
      title: 'Nothing changes until you switch',
      detail:
        'Existing profiles keep sending DNS through the VPN as before. You can switch in Profile settings, under DNS.',
    },
  ],
}

const SEEN_KEY = 'openwrap.whats-new.seen'

/** True when the current notes have not been dismissed on this machine. */
export function hasUnseenWhatsNew(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) !== WHATS_NEW.id
  } catch {
    // Without storage the window cannot be remembered, so do not nag.
    return false
  }
}

export function markWhatsNewSeen(): void {
  try {
    localStorage.setItem(SEEN_KEY, WHATS_NEW.id)
  } catch {
    // Nothing to do: the window simply shows again next launch.
  }
}
