# Helper Setup

OpenWrap's phase-2 macOS path uses `openwrap-helper` as a minimal privileged wrapper around the OpenVPN community binary.

Development setup:

1. Build the helper:
   `cargo build -p openwrap-helper`
2. Install the helper with root ownership and the setuid bit:
   `sudo chown root:wheel target/debug/openwrap-helper`
   `sudo chmod 4755 target/debug/openwrap-helper`
3. The `.env` file in the project root sets `OPENWRAP_HELPER_PATH` automatically.
   Source it before running:
   `source .env`

Verification:

1. Confirm the helper metadata:
   `ls -l target/debug/openwrap-helper`
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
