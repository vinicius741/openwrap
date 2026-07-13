# Helper Setup

OpenWrap's **default** macOS backend uses `openwrap-helper` as a minimal privileged wrapper around the OpenVPN community binary.

## Why the helper is the default (product decision)

This is a **hobby project**. The maintainer **does not pay** the Apple Developer Program fee for Network Extension / system-extension signing. Apple does not allow third-party Packet Tunnel system extensions to activate from free, ad-hoc, or unsigned builds.

So the supported day-to-day path is:

1. Build the app (no paid certificate required).
2. Install the setuid helper once with `sudo` (the only elevated step).
3. Connect using the OpenVPN community binary (e.g. Homebrew).

**Reviewers and agents:** do not reclassify this as “incomplete Packet Tunnel migration” or switch the default to Network Extension. That would make normal hobby installs unable to connect. See also `AGENTS.md` (“Hobby constraint”) and `src-tauri/src/app_state/backend_factory.rs`.

The optional Packet Tunnel path needs paid Apple entitlements; see [Packet Tunnel Setup](packet-tunnel-setup.md). Opt into it only when you have those:

```bash
export OPENWRAP_VPN_BACKEND=packet-tunnel
```

Optional custom helper path (must still be root-owned and setuid):

```bash
export OPENWRAP_HELPER_PATH=/path/to/openwrap-helper
```

## Manual installation

Build the app and helper without elevated privileges, then run the installer yourself from the repository root:

```bash
cargo build -p openwrap-helper --release
# or: npm run tauri:build
sudo ./scripts/install-helper.sh
```

Only the installer command requires `sudo`. Rerun it whenever `openwrap-helper` changes. The installer only accepts a binary named `openwrap-helper` under the repository `target/` tree.

## Development

1. Build the release helper: `cargo build -p openwrap-helper --release`.
2. Install it: `sudo ./scripts/install-helper.sh`.
3. Start the app: `npm run tauri:dev` (or open a normal unsigned build).

If `OPENWRAP_HELPER_PATH` is set, the app uses that path instead of the system installation. The path must still be root-owned and setuid. Note: `.env` is not loaded when you launch the app from Finder/Applications — only process environment variables apply.

Verification:

1. Confirm the helper metadata:
   `ls -l /Library/PrivilegedHelperTools/app.openwrap.desktop.openwrap-helper`.
   The mode should include `s` in the user-execute position and the owner should be `root`.
2. Run:
   `cargo check -p openwrap-helper`
3. Run the normal app/core checks:
   `cargo test -p openwrap-core`
   `cargo check -p openwrap-app`

Notes:

- The helper only accepts app-managed config/auth/runtime paths under `~/Library/Application Support/OpenWrap`.
- The helper launches OpenVPN with structured argv and a cleared environment.
- If the helper is missing, not root-owned, or not setuid, OpenWrap fails the connection attempt with a setup error instead of falling back to an unprivileged direct launch on macOS.
