# Architecture

OpenWrap is split into three layers:

1. `ui/` contains the React application and only knows about IPC contracts.
2. `src-tauri/` exposes Tauri commands, emits events, and owns tray lifecycle.
3. `crates/openwrap-core/` holds all business logic so the desktop shell stays thin.

The core crate defines traits for secret storage and VPN process launching. This keeps the Tauri layer free of platform-specific logic and makes the connection manager testable.

