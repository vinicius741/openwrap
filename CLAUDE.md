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

# Tests
npm test                 # Config regression tests (Node.js built-in runner)
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

2. **src-tauri/** — Thin shell exposing Tauri commands (in `commands/`), emitting events (in `events.rs`), and managing tray lifecycle (in `tray/`). Uses `AppState` (in `app_state/`) to hold `ConnectionManager` and repositories.

3. **crates/openwrap-core/** — All business logic including:
   - `connection/manager/` — ConnectionManager orchestrating VPN sessions via state machine (split into `connect.rs`, `events.rs`, `state.rs`, `runtime.rs`, `errors.rs`)
   - `connection/state_machine.rs` — State transitions (Idle → Validating → Connecting → Connected, etc.)
   - `profiles/` — Profile import, validation, storage
   - `config/` — OpenVPN config parsing and rewriting
   - `secrets/` — macOS Keychain integration
   - `dns/` — DNS observation and reconciliation
   - `logging/` — Session-based file logging for debugging
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

### Dev Server Configuration

The Vite dev server and Tauri dev URL are explicitly bound to `127.0.0.1` (not `localhost`) to avoid DNS resolution differences between Tauri's webview and Vite.

Because npm hoists `zustand` to root `node_modules/` while `react` stays in `ui/node_modules/`, the Vite config includes aliases that bridge this gap:

```ts
// ui/vite.config.ts
resolve: {
  alias: {
    react: './ui/node_modules/react',
    'react-dom': './ui/node_modules/react-dom',
  },
},
```

Theme initialization in `ui/src/main.tsx` is fire-and-forget (`void initTheme().catch(...)`) so that a font loading failure never blocks React from mounting (which causes a blank white screen).

Regression tests in `test/config-regression.mjs` guard all three of these constraints.

### Commands (Frontend → Backend)

Frontend calls commands via `invokeCommand<T>('command_name', { args })` defined in `ui/src/features/*/api.ts`.

Available commands are registered in `src-tauri/src/lib.rs`:
- `import_profile`, `list_profiles`, `get_profile`, `delete_profile`
- `connect`, `disconnect`, `submit_credentials`, `get_connection_state`
- `get_settings`, `update_settings`, `detect_openvpn`
- `reveal_logs_folder`, `get_recent_sessions`, `cleanup_old_logs`

### Events (Backend → Frontend)

Backend emits events defined in `src-tauri/src/events.rs`:
- `connection://state-changed` — Connection state updates
- `connection://log-line` — Sanitized log entries
- `connection://credentials-requested` — Credential prompt request
- `connection://dns-observed` — DNS configuration changes

Frontend listens via `useConnectionEvents()` hook in `ui/src/features/connection/useConnectionEvents.ts`.

## Frontend State Management

- **Zustand** (`ui/src/store/`) — Store split into focused slices and actions
  - `appStore.ts` — Store assembly only
  - `createAppStore.ts` — Store creation with all slices
  - `slices/` — `profileSlice.ts`, `connectionSlice.ts`, `settingsSlice.ts`, `importSlice.ts`
  - `actions/` — `loadInitial.ts`, `profileActions.ts`, `connectionActions.ts`
  - `reducers/` — Event handlers for backend events
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

- **SQLite** (`crates/openwrap-core/src/storage/sqlite/`) — Profiles, settings (split into `profile_queries.rs`, `mappers.rs`, `codec.rs`, `schema.rs`)
- **macOS Keychain** (`crates/openwrap-core/src/secrets/keychain.rs`) — Usernames only
- **Session Logs** (`crates/openwrap-core/src/logging/`) — Connection debugging logs
- Base directory: `~/Library/Application Support/OpenWrap/`

### Session Logs Structure

```
~/Library/Application Support/OpenWrap/
└── logs/
    ├── sessions/
    │   └── 2024-01-15/
    │       └── session-{uuid}/
    │           ├── metadata.json    # Session info (profile, times, outcome)
    │           ├── openvpn.log      # Raw OpenVPN output
    │           ├── dns.log          # DNS observations and changes
    │           └── core.log         # State transitions and events
    └── last-failed-openvpn.log      # Most recent failure for UI display
```

Session logs are organized by date and include:
- **metadata.json**: Profile name, start/end times, outcome (success/failed/cancelled)
- **openvpn.log**: All stdout/stderr from OpenVPN process
- **dns.log**: DNS configuration changes, auto-promotion events
- **core.log**: State machine transitions, PID info, exit codes

Enable verbose logging in Settings for immediate flush (useful for debugging crashes).

## File Naming Conventions

- **Rust**: `snake_case.rs` files, `snake_case` modules
- **TypeScript/React**: `kebab-case.tsx` for components, `kebab-case.ts` for utilities
- **Types**: `PascalCase` for interfaces/types in `types/ipc.ts`
