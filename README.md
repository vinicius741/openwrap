# OpenWrap

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform: macOS](https://img.shields.io/badge/Platform-macOS-lightgrey.svg)](https://www.apple.com/macos)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-blue.svg)](https://tauri.app)

OpenWrap is a lightweight macOS desktop client for OpenVPN profiles. Built with Tauri 2, it combines a React frontend with a Rust core that handles validation, import, persistence, secrets, and connection orchestration.

**Hobby project:** the default VPN path uses a free setuid helper and the OpenVPN community client so you **do not** need a paid Apple Developer Program membership. An optional Packet Tunnel / Network Extension backend exists in the tree but is **opt-in only** (requires paid NE entitlements). That split is intentional — see `AGENTS.md` (“Hobby constraint”) and [Helper Setup](docs/helper-setup.md).

## Features

- **Profile Management** — Import, validate, store, and list OpenVPN profiles
- **macOS Integration** — Native system tray with connection state and profile selection
- **Secure Storage** — Native macOS Keychain storage for remembered usernames, plus app-local generated-password secrets for opt-in PIN+TOTP profiles
- **DNS Management** — Apply full or split DNS with crash-recovery reconciliation (helper path)
- **Session Logging** — Persistent logs for debugging connection issues, organized by date
- **Privileged Helper** — Launch the OpenVPN community client without paid Apple entitlements
- **Optional Packet Tunnel** — System extension path when you have paid Network Extension signing

## Prerequisites

- macOS (primary platform)
- [Rust](https://rustup.rs/) (edition 2021)
- [Node.js](https://nodejs.org/) 18+ and npm
- [OpenVPN](https://openvpn.net/community-downloads/) community client (`brew install openvpn`) for the default helper path

## Development Setup

### 1. Install Dependencies

```bash
# Install Node.js dependencies
npm install

# Install Tauri CLI v2
cargo install tauri-cli --version "^2.0.0" --locked
```

### 2. Build OpenWrap and the helper

```bash
npm run tauri:build
```

### 3. Install the privileged helper

This is the only step that needs administrator privileges (one-time / after helper rebuilds):

```bash
sudo ./scripts/install-helper.sh
```

The installer only accepts `openwrap-helper` under the repository `target/` tree and installs it as `root:wheel` mode `4755`. Details: [Helper Setup](docs/helper-setup.md).

### 4. Start the app

```bash
npm run tauri:dev
# or open the built bundle:
open target/release/bundle/macos/OpenWrap.app
```

OpenWrap detects Homebrew OpenVPN at `/opt/homebrew/sbin/openvpn` or `/usr/local/sbin/openvpn`.

### Optional: Packet Tunnel (paid Apple program)

Requires Network Extension entitlements. See [Packet Tunnel Setup](docs/packet-tunnel-setup.md).

## Build the App Bundle

```bash
npm run tauri:build
```

The app bundle is created at `target/release/bundle/macos/OpenWrap.app`. No Apple Developer Program membership is required for the default helper path. After rebuilding the helper, rerun `sudo ./scripts/install-helper.sh`.

## Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start React dev server (UI only) |
| `npm run build` | Build the React frontend |
| `npm run tauri:dev` | Start Tauri development server |
| `npm run tauri:build` | Build the app and bundled privileged helper |
| `npm run tauri:build:signed` | Optional: signed Packet Tunnel build (paid Apple NE) |
| `sudo ./scripts/install-helper.sh` | Install or update the helper after a build |
| `npm test` | Run config regression tests |
| `npm run cargo:test` | Run Rust tests for openwrap-core |
| `npm run check` | Build UI and run Rust tests |

## Architecture

```
OpenWrap
├── ui/                    # React frontend (IPC contracts only)
├── src-tauri/             # Tauri commands, events, tray lifecycle
│   └── native/macos/      # Optional Packet Tunnel host bridge + provider
└── crates/
    ├── openwrap-core/     # Business logic, traits, connection manager
    └── openwrap-helper/   # Root-owned OpenVPN launcher and DNS reconciler
```

The core crate defines traits for secret storage and VPN process launching, keeping the Tauri layer thin and the connection manager testable.

## Documentation

- [Architecture Overview](docs/architecture.md) — Layer responsibilities and design
- [Security Model](docs/security-model.md) — Credential handling and trust boundaries
- [Helper Setup](docs/helper-setup.md) — Default privileged helper installation
- [Packet Tunnel Setup](docs/packet-tunnel-setup.md) — Optional Network Extension path (paid Apple NE)
- [Profile Import](docs/profile-import.md) — Import flow and validation
- [Roadmap](docs/roadmap.md) — Planned features and improvements

## Security

- Standard prompt-based profiles store only remembered usernames in Keychain
- Opt-in generated-password profiles store their local PIN+TOTP secret material in a separate app-local SQLite database (`openwrap-secrets.sqlite3`) under the OpenWrap data directory
- Imported profiles are treated as untrusted input
- Unsupported directives are blocked by default
- Clear failure reports for missing files, path traversal attempts, and unsupported options

## Verification

```bash
# Rust tests
cargo test -p openwrap-core

# Rust compilation checks
cargo check -p openwrap-app
cargo check -p openwrap-helper

# Dev server and config regression tests
npm test

# Frontend build
npm run build --workspace ui
```


## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
