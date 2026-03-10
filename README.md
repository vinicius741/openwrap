# OpenWrap

OpenWrap is a lightweight macOS desktop client for OpenVPN profiles. It is built as a small Tauri 2 app with a React frontend and a Rust domain core that owns validation, import, persistence, secrets, and connection orchestration.

Current status:
- Greenfield MVP scaffold
- Profile import, validation, storage, and listing implemented
- macOS helper-backed OpenVPN launch path implemented behind the core `VpnBackend` abstraction
- Native macOS Keychain storage is used for remembered credentials
- DNS remains observe-only and is surfaced from profile intent plus parsed OpenVPN runtime pushes
- Tray state follows the current connection state and the last selected profile

Security notes:
- Credentials are never stored in SQLite or plaintext config files.
- Imported profiles are treated as untrusted input and unsupported directives are blocked by default.
- Import blocks now include clearer failure reports for missing files, path traversal attempts, duplicate managed assets, and unsupported DHCP options.

Verification:
- `cargo test -p openwrap-core`
- `cargo check -p openwrap-app`
- `cargo check -p openwrap-helper`
- `npm run build --workspace ui`

See [docs/architecture.md](/Users/ilia/Documents/openwrap/docs/architecture.md), [docs/security-model.md](/Users/ilia/Documents/openwrap/docs/security-model.md), and [docs/helper-setup.md](/Users/ilia/Documents/openwrap/docs/helper-setup.md) for details.
