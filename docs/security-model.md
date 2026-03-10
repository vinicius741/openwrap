# Security Model

- Imported `.ovpn` files are treated as untrusted input.
- Unknown directives are blocked by default.
- Script execution directives and management interface directives are blocked in v1.
- Credentials never enter SQLite; only Keychain references and booleans are persisted.
- OpenVPN is launched with structured argv and app-managed config paths only.
- Logs are sanitized before they are emitted to the UI.

