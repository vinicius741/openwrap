# Windows setup

OpenWrap on Windows launches the **OpenVPN Community** client (`openvpn.exe`)
directly. There is no setuid helper and no paid code signing: the app runs
**as administrator** (its manifest requests elevation, so Windows shows a UAC
prompt on launch) and OpenVPN inherits that elevation to create the tunnel
adapter and change routes and DNS.

> Status: experimental. The Windows code compiles and is covered by CI, but
> real connect/disconnect behavior still needs to be validated on Windows.
> See "Known limitations" below.

## Prerequisites

- Windows 10 or 11 (x64). WebView2 ships with Windows 11; on Windows 10 install
  the Evergreen runtime if it is missing.
- **OpenVPN Community 2.6+** from <https://openvpn.net/community-downloads/>
  (or `winget install --id OpenVPNTechnologies.OpenVPN --exact`). The default
  install includes the Wintun / DCO drivers OpenWrap relies on. OpenWrap finds
  `openvpn.exe` via `HKLM\SOFTWARE\OpenVPN` or
  `C:\Program Files\OpenVPN\bin\openvpn.exe`; a custom path can be set in
  Settings.

## Using a build

1. Download the unsigned installer (`OpenWrap_*_x64-setup.exe`) from the
   `openwrap-windows-installer` artifact of a CI run, or build it yourself (below).
2. SmartScreen will warn about an unrecognized app: choose **More info → Run anyway**.
3. Launch OpenWrap and accept the UAC prompt.
4. If OpenVPN is missing, **Install OpenVPN** in Settings runs winget in a console window.
5. Import a `.ovpn` profile and connect.

## Development

Requirements: Rust stable (MSVC toolchain + Visual Studio Build Tools with the
"Desktop development with C++" workload), Node 22, and
`cargo install tauri-cli --version "^2.0.0" --locked`.

Run everything from an **elevated** terminal (Run as administrator). The app
binary requests elevation, so `cargo tauri dev` from a normal terminal fails with
"The requested operation requires elevation" (os error 740).

```powershell
npm install
npm test
cargo test -p openwrap-core -p openwrap-app
npm run tauri:dev                       # dev app
npx @tauri-apps/cli@2 build --bundles nsis   # installer in target\release\bundle\nsis
```

`src-tauri/tauri.windows.conf.json` is merged automatically on Windows: it
skips building the Unix-only helper and builds an NSIS installer.

## How it works

| Area | Windows behavior |
|---|---|
| VPN backend | `DirectOpenVpnBackend` spawns `openvpn.exe` with no console window. Connect fails with a clear message if the app is not elevated. |
| Disconnect | OpenVPN is started with `--service <event> 0`. Disconnect signals that named event so OpenVPN shuts down cleanly (routes and DNS restored), then force-terminates it after 10 s if it is still running. |
| DNS | OpenVPN applies DNS itself. Split DNS (default): no extra directives. Full override: adds `block-outside-dns`. Observe only: `pull-filter ignore` for pushed DNS options. |
| Remember username | Windows Credential Manager, generic credential `app.openwrap.credentials:profile-<id>` (username only; passwords are never stored). |
| Data | `%LOCALAPPDATA%\OpenWrap` (profiles, logs, runtime files). |
| Reveal in file manager | `explorer.exe /select,<path>`. |
| Profile policy | `windows-driver` is allowed and `block-outside-dns` requires approval on Windows; both stay blocked on macOS. |

## Known limitations

- The app must run elevated. Start-at-login would need a scheduled task, which
  is not set up. Drag-and-drop from a non-elevated Explorer window into an
  elevated app is blocked by Windows (UIPI); use the import dialog.
- If OpenWrap crashes while connected, `openvpn.exe` keeps running. There is no
  startup cleanup on Windows yet; end it in Task Manager.
- The non-admin path through OpenVPN's Interactive Service (named pipe plus the
  management interface, as openvpn-gui does) is not implemented.
- Auto-update and release packaging for Windows are not wired into
  `release.yml` / `latest.json` yet.
