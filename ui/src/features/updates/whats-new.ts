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
  id: '2026-11-openvpn-repair',
  headline: 'OpenWrap can now repair OpenVPN',
  points: [
    {
      title: 'Fixes OpenVPN after a Homebrew update',
      detail:
        'If OpenVPN stops working, OpenWrap tells you and offers Repair OpenVPN, which updates it with Homebrew in Terminal.',
    },
    {
      title: 'Connection failing? Try Update OpenVPN',
      detail:
        'When a connection fails, the error now shows your OpenVPN version and an Update OpenVPN button. It is also in Settings.',
    },
    {
      title: 'Keep "All DNS through VPN" for now',
      detail:
        'The "Only listed domains" DNS mode from the previous version does not work yet. It will be fixed in a later update.',
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
