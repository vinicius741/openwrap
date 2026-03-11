# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OpenWrap is a macOS desktop client for OpenVPN profiles built with Tauri 2. It combines a React frontend (`ui/`) with a Rust backend (`src-tauri/` and `crates/openwrap-core/`).

## Development Commands

```bash
# Install dependencies
npm install
cargo install tauri-cli --version "^2.0.0" --locked

# Development
npm run dev              # React dev server only (UI only, no Tauri)
npm run tauri:dev        # Full Tauri development server

# Build
npm run build            # Build React frontend
npm run build --workspace ui  # Same as above

# Rust tests
npm run cargo:test       # Run openwrap-core tests
cargo test -p openwrap-core   # Direct

# Combined check
npm run check            # Build UI + run Rust tests

# Individual crate compilation
cargo check -p openwrap-app
cargo check -p openwrap-helper
cargo check -p openwrap-core
```

## Privileged Helper Setup

The helper is required for OpenVPN execution with root privileges:

```bash
cargo build -p openwrap-helper
sudo chown root:wheel target/debug/openwrap-helper
sudo chmod 4755 target/debug/openwrap-helper
source .env  # Sets OPENWRAP_HELPER_PATH
```

## Architecture

```
OpenWrap
├── ui/                    # React frontend (IPC contracts only)
├── src-tauri/             # Tauri commands, events, tray lifecycle
└── crates/
    ├── openwrap-core/     # Business logic, traits, connection manager
    └── openwrap-helper/   # Privileged OpenVPN wrapper (setuid binary)
```

### Layer Responsibilities

1. **ui/** — React + Zustand + TypeScript. Only knows about IPC contracts defined in `ui/src/types/ipc.ts`. Calls Tauri commands via `invokeCommand()` in `ui/src/lib/tauri.ts`.

2. **src-tauri/** — Thin shell exposing Tauri commands (in `commands/`), emitting events (in `events.rs`), and managing tray lifecycle. Uses `AppState` to hold `ConnectionManager` and repositories.

3. **crates/openwrap-core/** — All business logic including:
   - `connection/manager.rs` — ConnectionManager orchestrates VPN sessions via state machine
   - `connection/state_machine.rs` — State transitions (Idle → Validating → Connecting → Connected, etc.)
   - `profiles/` — Profile import, validation, storage
   - `config/` — OpenVPN config parsing and rewriting
   - `secrets/` — macOS Keychain integration
   - `dns/` — DNS observation and reconciliation
   - `openvpn/` — Process launching via helper or direct

### Key Traits (openwrap-core)

```rust
pub trait SecretStore: Send + Sync {
    fn get_password(&self, profile_id: &ProfileId) -> Result<Option<StoredSecret>, AppError>;
    fn set_password(&self, secret: StoredSecret) -> Result<(), AppError>;
    fn delete_password(&self, profile_id: &ProfileId) -> Result<(), AppError>;
}

pub trait VpnBackend: Send + Sync {
    fn connect(&self, request: ConnectRequest) -> Result<SpawnedSession, AppError>;
    fn disconnect(&self, session_id: SessionId) -> Result<(), AppError>;
    fn reconcile_dns(&self, request: ReconcileDnsRequest) -> Result<(), AppError>;
}
```

These traits are defined in `crates/openwrap-core/src/lib.rs` and keep the Tauri layer testable.

## IPC Patterns

### Commands (Frontend → Backend)

Frontend calls commands via `invokeCommand<T>('command_name', { args })` defined in `ui/src/features/*/api.ts`.

Available commands are registered in `src-tauri/src/lib.rs`:
- `import_profile`, `list_profiles`, `get_profile`, `delete_profile`
- `connect`, `disconnect`, `submit_credentials`, `get_connection_state`
- `get_settings`, `update_settings`, `detect_openvpn`

### Events (Backend → Frontend)

Backend emits events defined in `src-tauri/src/events.rs`:
- `connection://state-changed` — Connection state updates
- `connection://log-line` — Sanitized log entries
- `connection://credentials-requested` — Credential prompt request
- `connection://dns-observed` — DNS configuration changes

Frontend listens via `useConnectionEvents()` hook in `ui/src/features/connection/useConnectionEvents.ts`.

## Frontend State Management

- **Zustand** (`ui/src/store/appStore.ts`) — Single store for all app state
- State is updated via store actions that call API functions
- Events from backend directly update store via `setConnection`, `appendLog`, etc.

## Connection State Machine

States defined in `crates/openwrap-core/src/connection/session.rs`:
```
Idle → ValidatingProfile → [AwaitingCredentials] → PreparingRuntime → StartingProcess → Connecting → Connected
                                                                                                      ↓
                                                                                               Reconnecting ← (on retry)
                                                                                                      ↓
                                                                                              Disconnecting → Idle
```

Transitions are handled by `state_machine.rs` via `transition(current, intent)` function.

## Security Model

- Imported `.ovpn` files are treated as untrusted input
- Unknown directives are blocked by default
- Passwords are **never** stored — only usernames via macOS Keychain
- OpenVPN is launched with structured argv and app-managed paths only
- Logs are sanitized before emission to UI

## Data Storage

- **SQLite** (`crates/openwrap-core/src/storage/sqlite.rs`) — Profiles, settings
- **macOS Keychain** (`crates/openwrap-core/src/secrets/keychain.rs`) — Usernames only
- Base directory: `~/Library/Application Support/OpenWrap/`

## File Naming Conventions

- **Rust**: `snake_case.rs` files, `snake_case` modules
- **TypeScript/React**: `kebab-case.tsx` for components, `kebab-case.ts` for utilities
- **Types**: `PascalCase` for interfaces/types in `types/ipc.ts`
