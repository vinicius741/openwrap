# Security Model

## Profiles and secrets

- Imported `.ovpn` files are treated as untrusted input.
- Directives are checked against an allow-list at import; unknown directives
  are blocked.
- Script execution directives and management interface directives are
  blocked in imported profiles.
- Referenced assets must stay inside the imported profile's directory,
  including through symlinks.
- Standard prompt-based profiles only persist saved usernames in Keychain
  (Credential Manager on Windows).
- Profiles configured for generated PIN+TOTP passwords store the PIN and TOTP
  secret in a separate app-local SQLite database (`openwrap-secrets.sqlite3`,
  mode 0600) under the OpenWrap app data directory, not in Keychain.
- The password for a session is written to a 0600 `auth.txt` in the session's
  runtime directory and removed when the session ends.
- Log lines shown in the UI have the literal words `password`/`PASSWORD`
  masked. This is cosmetic: OpenVPN does not print credentials at the log
  level OpenWrap uses, and raw OpenVPN output is kept in the session log files.

## macOS privileged helper (default backend)

The Tauri app runs unprivileged. A setuid-root helper
(`/Library/PrivilegedHelperTools/app.openwrap.desktop.openwrap-helper`) starts
OpenVPN and restores DNS. This is the free path; see `AGENTS.md`
("Hobby constraint").

What the helper enforces:

- It only serves the account that installed it. Installing the helper from
  OpenWrap writes that account's uid to a root-owned file next to it
  (`app.openwrap.desktop.openwrap-helper.owner`); every other non-root
  account is refused (exit status 77). Another account on the same Mac that
  wants to use OpenWrap must install the helper again, with an administrator
  password, which moves it to that account.
- A helper installed by an older OpenWrap (no owner file) instead serves
  whoever is logged in at the Mac's screen (owner of `/dev/console`), which
  includes any other account that logs in there. OpenWrap offers to reinstall
  such a helper.
- It only launches OpenVPN from known install locations (Homebrew, MacPorts).
- Config, auth and runtime paths must be under the calling user's
  `~/Library/Application Support/OpenWrap`.
- OpenVPN gets structured argv and a cleared environment, never a shell.
- DNS reconciliation refuses symlinked or hard-linked state files, only
  deletes OpenWrap resolver files under `/etc/resolver/`, and only removes
  host routes to IP addresses.

What it does **not** protect against: **any program running as the account
the helper serves can get unrestricted root without a password.** That is
more than "passwordless `sudo` for OpenVPN": OpenVPN runs as root with a
config the account controls, and OpenVPN config can load plugins and run
scripts. Concretely:

- The launch config and `auth.txt` live in a directory that account can
  write, and root OpenVPN reads them without the helper checking their
  content.
- The DNS up/down scripts are written to that account's temporary directory
  and run as root.
- Homebrew's OpenVPN (and the libraries it loads) are owned by that account
  on Apple Silicon.

The first two do not depend on who owns the OpenVPN binary, so a root-owned
OpenVPN (for example from MacPorts) alone does not close this. Closing it
would need the helper to generate or fully validate the config and scripts
itself and to run only a root-owned OpenVPN, which this hobby project does not
do. Treat installing the helper like giving that account passwordless `sudo`.

- Helper-path DNS changes have persisted restore state and are reconciled
  after interrupted sessions and at the next launch.

## Windows (experimental)

The app runs elevated (`requireAdministrator`) and starts `openvpn.exe`
directly. There is no separate privilege boundary inside the app, and
**any program running as the same Windows user can get administrator rights
through OpenWrap** without a UAC prompt:

- Profiles, launch configs and settings live under `%LOCALAPPDATA%\OpenWrap`,
  which that user can write. The elevated `openvpn.exe` runs whatever the
  launch config says, and the OpenVPN path override in the settings database
  is run elevated. OpenWrap re-checks the stored profile at connect, but the
  same user can change the files between that check and OpenVPN reading them.
- In-app updates: the updater verifies the installer's signature in memory,
  then writes it to the user's `%TEMP%` and runs it elevated. A program
  running as that user could replace the file in between.

Closing this needs OpenVPN's interactive service (non-admin) route, which is
not implemented (see `docs/windows-setup.md`).

## Optional Packet Tunnel (paid Apple Network Extension signing)

Runs OpenVPN 3 Core inside a system extension; profile contents and
credentials are passed as ephemeral start options only. DNS is applied only
through `NEPacketTunnelNetworkSettings` and is torn down with the tunnel.
