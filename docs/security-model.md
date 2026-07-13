# Security Model

- Imported `.ovpn` files are treated as untrusted input.
- Unknown directives are blocked by default.
- Script execution directives and management interface directives are blocked in v1.
- Standard prompt-based profiles only persist saved usernames in Keychain.
- Profiles explicitly configured for generated PIN+TOTP passwords store that local secret material in a separate app-local SQLite database (`openwrap-secrets.sqlite3`) under the OpenWrap app data directory.
- On macOS, the default helper path keeps the Tauri UI unprivileged. Only the small root-owned helper and its OpenVPN child run elevated. The helper accepts structured requests, validates app-managed paths, clears the child environment, and never invokes a shell for connections.
- The optional Packet Tunnel path (paid Apple Network Extension signing) runs OpenVPN 3 Core inside a system extension; profile contents and credentials are ephemeral start options only.
- Temporary credentials use restricted runtime files and are removed during disconnect cleanup.
- Helper-path DNS changes have persisted restore state and are reconciled after interrupted sessions. Packet Tunnel DNS is applied only through `NEPacketTunnelNetworkSettings` and is torn down with the tunnel.
- Logs are sanitized before they are emitted to the UI.
