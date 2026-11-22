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

- It only runs for the user logged in at the Mac's screen (owner of
  `/dev/console`). Other local accounts, SSH sessions and background accounts
  cannot use it.
- It only launches OpenVPN from known install locations (Homebrew, MacPorts).
- Config, auth and runtime paths must be under the calling user's
  `~/Library/Application Support/OpenWrap`.
- OpenVPN gets structured argv and a cleared environment, never a shell.
- DNS reconciliation refuses symlinked state files, only deletes OpenWrap
  resolver files under `/etc/resolver/`, and only removes host routes to IP
  addresses.

What it does **not** protect against: **the logged-in user's own programs can
get root without a password.** The config file and the DNS scripts OpenVPN
runs live in directories that user can write, and Homebrew's OpenVPN (and the
libraries it loads) are owned by that user on Apple Silicon. Any process
running as you could change them and then ask the helper to start OpenVPN.
Closing that would mean shipping and maintaining a root-owned OpenVPN build,
which this hobby project deliberately does not do. Treat installing the
helper like giving your account passwordless `sudo` for OpenVPN.

- Helper-path DNS changes have persisted restore state and are reconciled
  after interrupted sessions and at the next launch.

## Windows (experimental)

The app runs elevated (`requireAdministrator`) and starts `openvpn.exe`
directly. There is no separate privilege boundary inside the app.

## Optional Packet Tunnel (paid Apple Network Extension signing)

Runs OpenVPN 3 Core inside a system extension; profile contents and
credentials are passed as ephemeral start options only. DNS is applied only
through `NEPacketTunnelNetworkSettings` and is torn down with the tunnel.
