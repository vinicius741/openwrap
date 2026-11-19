# Windows setup

OpenWrap on Windows launches the **OpenVPN Community** client (`openvpn.exe`)
directly. There is no setuid helper and no paid code signing: the app runs
**as administrator** (its manifest requests elevation, so Windows shows a UAC
prompt on launch) and OpenVPN inherits that elevation to create the tunnel
adapter and change routes and DNS.

> Status: experimental. Core Windows flows (OpenVPN 2.7 DCO connect/disconnect
> via `--service`, routes and DNS apply/restore, `block-outside-dns` WFP
> filters, `ObserveOnly` pull filters, Credential Manager username storage,
> manifest elevation, and NSIS silent install/uninstall) are validated in
> `windows-latest` CI. Interactive desktop GUI flows (UAC prompt UX, tray
> interactions, WebView2 rendering, and interactive Explorer reveal) still need
> manual testing on a Windows desktop. See "Known limitations" below.

## Prerequisites

- Windows 10 or 11 (x64). WebView2 ships with Windows 11; on Windows 10 the
  installer downloads it if it is missing (Tauri's default bootstrapper).
- **OpenVPN Community 2.6+**. The OpenWrap installer offers to install it (see
  below); otherwise get it from <https://openvpn.net/community-downloads/>
  (or `winget install --id OpenVPNTechnologies.OpenVPN --exact`). The default
  install includes the DCO / TAP network drivers OpenWrap relies on. OpenWrap
  finds `openvpn.exe` via `HKLM\SOFTWARE\OpenVPN` or
  `C:\Program Files\OpenVPN\bin\openvpn.exe`; a custom path can be set in
  Settings.

## Using a build

1. Download `OpenWrap-Windows-Setup.exe` from the latest release:
   <https://github.com/vinicius741/openwrap/releases/latest/download/OpenWrap-Windows-Setup.exe>
   (or the `openwrap-windows-installer` artifact of a CI run, or build it yourself, below).
2. SmartScreen warns about an unrecognized app because the installer is unsigned:
   choose **More info → Run anyway**, then accept the administrator prompt.
3. If OpenVPN is not installed, the installer asks to install it. Choose **Yes**:
   it downloads the pinned OpenVPN Community MSI from openvpn.net, checks its
   SHA-256 and installs it with a progress bar (no wizard).
4. Launch OpenWrap and accept the UAC prompt. If OpenVPN is still missing (you
   chose No, or the download failed), a banner offers **Install OpenVPN**, which
   runs the same script in a console window.
5. Import a `.ovpn` profile and connect.

To update, run the newer `OpenWrap-Windows-Setup.exe` over the existing install.

### OpenVPN setup script

`src-tauri/windows/install-openvpn.ps1` is bundled next to `openwrap-app.exe`
and used by both the NSIS post-install hook (`hooks.nsh`) and the in-app
button. It pins one OpenVPN version, URL and SHA-256; bump all three together.
CI installs OpenVPN with it on every push, so a stale pin fails CI. Silent
installs (`/S`) skip the OpenVPN prompt.

## Development

Requirements: Rust stable (MSVC toolchain + Visual Studio Build Tools with the
"Desktop development with C++" workload), Node 22, and
`cargo install tauri-cli --version "^2.3.0" --locked` (Tauri CLI 2.3+ is
required for `NSIS_HOOK_PREFINISHPAGE` in `src-tauri/windows/hooks.nsh`).

Run everything from an **elevated** terminal (Run as administrator). The app
binary requests elevation, so `cargo tauri dev` from a normal terminal fails with
"The requested operation requires elevation" (os error 740).

```powershell
npm install
npm test
cargo test -p openwrap-core -p openwrap-app
cargo test -p openwrap-core --test windows_openvpn_e2e -- --ignored --nocapture  # real OpenVPN E2E
npm run tauri:dev                       # dev app
npx @tauri-apps/cli@2 build --bundles nsis   # installer in target\release\bundle\nsis
```

`src-tauri/tauri.windows.conf.json` is merged automatically on Windows: it
skips building the Unix-only helper and builds a per-machine NSIS installer
(`C:\Program Files\OpenWrap`).

## How it works

| Area | Windows behavior |
|---|---|
| Installer | Per-machine NSIS installer into `C:\Program Files\OpenWrap` (admin-writable), separate from app data in `%LOCALAPPDATA%\OpenWrap`. |
| VPN backend | `DirectOpenVpnBackend` spawns `openvpn.exe` with no console window and assigns it to a `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` Job Object so `openvpn.exe` cannot be orphaned if OpenWrap crashes. Connect fails with a clear message if the app is not elevated. |
| Disconnect | OpenVPN is started with `--service <event> 0`. Disconnect signals that named event so OpenVPN shuts down cleanly (routes and DNS restored), then force-terminates it after 10 s (5 s on app quit) if it is still running. Stale `runtime/` session files are also swept on startup. |
| DNS | OpenVPN applies DNS itself. Full override (default): adds `block-outside-dns` (WFP). Split DNS (VPN adapter): strips any profile-local `block-outside-dns` and adds `pull-filter ignore "block-outside-dns"`. Observe only: strips profile-local DNS directives and adds `pull-filter ignore` for pushed DNS options. |
| Remember username | Windows Credential Manager, generic credential `app.openwrap.credentials:profile-<id>` (username only; passwords are never stored). |
| Data | `%LOCALAPPDATA%\OpenWrap` (profiles, logs, runtime files). |
| Reveal in file manager | `%SystemRoot%\explorer.exe /select,"<path>"` (using raw Windows command-line quoting so paths with spaces and UNC paths work). |
| Profile policy | `windows-driver` is allowed and `block-outside-dns` requires approval on Windows; both stay blocked on macOS. |

## Known limitations

- **Elevated-app trust boundary**: OpenWrap's executable installs into
  admin-only `C:\Program Files\OpenWrap`, but imported profiles, logs, and the
  optional OpenVPN binary path override live in user-writable
  `%LOCALAPPDATA%\OpenWrap`. Any unelevated process running as the same Windows
  user can modify files in `%LOCALAPPDATA%\OpenWrap`. This is inherent to the
  elevated-app design until an unprivileged GUI + Interactive Service backend is
  implemented.
- **Over-the-shoulder UAC**: If a standard user approves UAC with a different
  administrator account, OpenWrap runs under that admin account, so profiles and
  data are stored under the admin account's `%LOCALAPPDATA%\OpenWrap` (and
  `winget` may not be registered for that account).
- The app must run elevated. Start-at-login would need a scheduled task, which
  is not set up. Drag-and-drop from a non-elevated Explorer window into an
  elevated app is blocked by Windows (UIPI); use the import dialog.
- If OpenWrap is force-killed abruptly while connected, Windows terminates
  `openvpn.exe` via the Job Object, which may leave tunnel routes or NRPT rules
  in place until reconnecting or rebooting; stale `runtime/` files (including
  any leftover `auth.txt`) are cleaned on next launch.
- The non-admin path through OpenVPN's Interactive Service (named pipe plus the
  management interface, as openvpn-gui does) is not implemented.
- Auto-update for Windows is not wired into `latest.json` yet (the release
  workflow only attaches `OpenWrap-Windows-Setup.exe`). Wiring the NSIS updater later will also
  require overriding `.onInstSuccess` relaunch for the `requireAdministrator`
  manifest, just as `hooks.nsh` overrides the finish page).
