# Security Model

- Imported `.ovpn` files are treated as untrusted input.
- Unknown directives are blocked by default.
- Script execution directives and management interface directives are blocked in v1.
- Standard prompt-based profiles only persist saved usernames in Keychain.
- Profiles explicitly configured for generated PIN+TOTP passwords store that local secret material in a separate app-local SQLite database under the OpenWrap app data directory.
- OpenVPN is launched with structured argv and app-managed config paths only.
- Logs are sanitized before they are emitted to the UI.
