# Architecture

OpenWrap is split into three layers:

1. `ui/` contains the React application and only knows about IPC contracts.
2. `src-tauri/` exposes Tauri commands, emits events, and owns tray lifecycle.
3. `crates/openwrap-core/` holds all business logic so the desktop shell stays thin.

The core crate defines traits for secret storage and VPN process launching. This keeps the Tauri layer free of platform-specific logic and makes the connection manager testable.

## Data Storage

OpenWrap stores application data under `~/Library/Application Support/OpenWrap/` (managed via `AppPaths`):

- **`openwrap.sqlite3`**: The main SQLite database, storing imported OpenVPN profiles, settings, and metadata.
- **`openwrap-secrets.sqlite3`**: A separate, restricted SQLite database storing local credentials (PINs and TOTP secret keys) for generated password authentication.
- **Keychain**: Native macOS Keychain integration is used to store usernames for standard prompt-based authentication profiles.
- **`runtime/`**: Transient directory for active VPN session files (e.g., rewritten config files, credentials temporary buffers).
- **`logs/`**: Session logs directory, organized by date, saving debug logs for OpenVPN processes, core transitions, and DNS observations.

## App Exit & Shutdown Lifecycle

To prevent stale configuration and orphaned files, the Tauri run loop handles `ExitRequested` and `Exit` events to trigger `AppState::shutdown()`. The shutdown procedure uses an atomic guard to safely:
1. Reconcile and restore system DNS configurations.
2. Clean up any active VPN connection sessions and run loops.
3. Delete all transient files inside the `runtime/` directory.

## Dev Server & Module Resolution

In development, the Vite dev server binds to `127.0.0.1` (instead of `localhost`) for consistency with Tauri's webview. In addition, Vite aliases map React and React DOM directly to prevent duplicate React runtime instances caused by hoisted dependencies (like `zustand`). These conditions are automatically validated by configuration regression tests.


