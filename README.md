# OpenWrap

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform: macOS](https://img.shields.io/badge/Platform-macOS-lightgrey.svg)](https://www.apple.com/macos)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-blue.svg)](https://tauri.app)

OpenWrap is a lightweight macOS desktop client for OpenVPN profiles. Built with Tauri 2, it combines a React frontend with a Rust core that handles validation, import, persistence, secrets, and connection orchestration.

## Features

- **Profile Management** — Import, validate, store, and list OpenVPN profiles
- **macOS Integration** — Native system tray with connection state and profile selection
- **Secure Storage** — Native macOS Keychain storage for remembered usernames
- **DNS Observation** — Surface DNS from profile intent and OpenVPN runtime pushes
- **Session Logging** — Persistent logs for debugging connection issues, organized by date
- **Privileged Helper** — Secure OpenVPN execution via setuid helper wrapper

## Prerequisites

- macOS (primary platform)
- [Rust](https://rustup.rs/) (edition 2021)
- [Node.js](https://nodejs.org/) 18+ and npm
- [OpenVPN](https://openvpn.net/) (community binary)

## Development Setup

### 1. Install Dependencies

```bash
# Install Node.js dependencies
npm install

# Install Tauri CLI v2
cargo install tauri-cli --version "^2.0.0" --locked
```

### 2. Build the Privileged Helper

The helper wrapper is required to launch OpenVPN with root privileges:

```bash
cargo build -p openwrap-helper
sudo chown root:wheel target/debug/openwrap-helper
sudo chmod 4755 target/debug/openwrap-helper
```

### 3. Configure Environment

```bash
source .env
```

### 4. Start Development Server

```bash
npm run tauri:dev
```

## Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start React dev server (UI only) |
| `npm run build` | Build the React frontend |
| `npm run tauri:dev` | Start Tauri development server |
| `npm run cargo:test` | Run Rust tests for openwrap-core |
| `npm run check` | Build UI and run Rust tests |

## Architecture

```
OpenWrap
├── ui/                    # React frontend (IPC contracts only)
├── src-tauri/             # Tauri commands, events, tray lifecycle
└── crates/
    ├── openwrap-core/     # Business logic, traits, connection manager
    └── openwrap-helper/   # Privileged OpenVPN wrapper (macOS)
```

The core crate defines traits for secret storage and VPN process launching, keeping the Tauri layer thin and the connection manager testable.

## Documentation

- [Architecture Overview](docs/architecture.md) — Layer responsibilities and design
- [Security Model](docs/security-model.md) — Credential handling and trust boundaries
- [Helper Setup](docs/helper-setup.md) — Privileged wrapper configuration
- [Profile Import](docs/profile-import.md) — Import flow and validation
- [Roadmap](docs/roadmap.md) — Planned features and improvements

## Security

- Passwords are **never** stored in SQLite, Keychain, or plaintext config files
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

# Frontend build
npm run build --workspace ui
```

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
