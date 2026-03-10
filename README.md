# OpenWrap

OpenWrap is a lightweight macOS desktop client for OpenVPN profiles. It is built as a small Tauri 2 app with a React frontend and a Rust domain core that owns validation, import, persistence, secrets, and connection orchestration.

Current status:
- Greenfield MVP scaffold
- Profile import, validation, storage, and listing implemented
- macOS helper-backed OpenVPN launch path implemented behind the core `VpnBackend` abstraction
- Native macOS Keychain storage is used for remembered usernames only
- DNS remains observe-only and is surfaced from profile intent plus parsed OpenVPN runtime pushes
- Tray state follows the current connection state and the last selected profile

Security notes:
- Passwords are never stored in SQLite, Keychain, or plaintext config files.
- Imported profiles are treated as untrusted input and unsupported directives are blocked by default.
- Import blocks now include clearer failure reports for missing files, path traversal attempts, duplicate managed assets, and unsupported DHCP options.

Verification:
- `cargo test -p openwrap-core`
- `cargo check -p openwrap-app`
- `cargo check -p openwrap-helper`
- `npm run build --workspace ui`

## Development Setup

To run the OpenWrap application locally, you must compile the macOS helper wrapper and set the required root permissions in addition to starting the Tauri development server.

1. **Build the OpenVPN privileged wrapper (helper):**
   ```bash
   cargo build -p openwrap-helper
   ```

2. **Allow the helper to execute with root privileges:**
   ```bash
   sudo chown root:wheel target/debug/openwrap-helper
   sudo chmod 4755 target/debug/openwrap-helper
   ```

3. **Source the environment variables:**
   ```bash
   source .env
   ```

4. **Install the Tauri CLI (if not already installed):**
   This project uses Tauri v2, so you'll need the v2 CLI via Cargo.
   ```bash
   cargo install tauri-cli --version "^2.0.0" --locked
   ```

5. **Start the Tauri development server:**
   ```bash
   npm run tauri:dev
   ```

See [docs/architecture.md](/Users/ilia/Documents/openwrap/docs/architecture.md), [docs/security-model.md](/Users/ilia/Documents/openwrap/docs/security-model.md), and [docs/helper-setup.md](/Users/ilia/Documents/openwrap/docs/helper-setup.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
