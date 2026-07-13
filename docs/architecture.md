# Architecture

OpenWrap is split into three layers:

1. `ui/` contains the React application and only knows about IPC contracts.
2. `src-tauri/` exposes Tauri commands, emits events, owns tray lifecycle, and selects the macOS VPN backend (helper by default).
3. `crates/openwrap-core/` holds all business logic so the desktop shell stays thin.

The core crate defines traits for secret storage and VPN process launching. This keeps the Tauri layer free of platform-specific logic and makes the connection manager testable.

## Data Storage

OpenWrap stores application data under `~/Library/Application Support/OpenWrap/` (managed via `AppPaths`):

- **`openwrap.sqlite3`**: The main SQLite database, storing imported OpenVPN profiles, settings, and metadata.
- **`openwrap-secrets.sqlite3`**: A separate, restricted SQLite database storing local credentials (PINs and TOTP secret keys) for generated password authentication.
- **Keychain**: Native macOS Keychain integration is used to store usernames for standard prompt-based authentication profiles.
- **`runtime/`**: Transient directory for active VPN session files (e.g., rewritten config files, credentials temporary buffers).
- **`logs/`**: Session logs directory, organized by date, saving debug logs for OpenVPN processes, core transitions, and DNS observations.

## macOS VPN backends

### Privileged helper (default)

`HelperOpenVpnBackend` sends a validated, structured request to the root-owned `openwrap-helper`, which launches the OpenVPN community binary. This is the **intentional hobby default** (no paid Apple Network Extension entitlement — the maintainer does not pay the Developer Program fee for this project). It uses OpenWrap-managed DNS scripts and startup reconciliation for crash recovery. See `AGENTS.md` (“Hobby constraint”).

### Packet Tunnel (opt-in only)

When `OPENWRAP_VPN_BACKEND=packet-tunnel` is set, `NetworkExtensionBackend` activates an embedded Packet Tunnel system extension. That path needs a paid, provisioned Apple signature. It is **not** incomplete work when left off by default; defaulting to helper is a product constraint, not a temporary gap.

## Dev Server & Module Resolution

In development, the Vite dev server binds to `127.0.0.1` (instead of `localhost`) for consistency with Tauri's webview. In addition, Vite aliases map React and React DOM directly to prevent duplicate React runtime instances caused by hoisted dependencies (like `zustand`). These conditions are automatically validated by configuration regression tests.
