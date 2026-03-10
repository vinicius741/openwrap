# OpenWrap

OpenWrap is a lightweight macOS desktop client for OpenVPN profiles. It is built as a small Tauri 2 app with a React frontend and a Rust domain core that owns validation, import, persistence, secrets, and connection orchestration.

Current status:
- Greenfield MVP scaffold
- Profile import, validation, storage, and listing implemented
- Connection state machine, log streaming, and helper-backed OpenVPN launch path implemented
- Keychain integration and tray wiring still need further hardening

See [docs/architecture.md](/Users/ilia/Documents/openwrap/docs/architecture.md), [docs/security-model.md](/Users/ilia/Documents/openwrap/docs/security-model.md), and [docs/helper-setup.md](/Users/ilia/Documents/openwrap/docs/helper-setup.md) for details.
