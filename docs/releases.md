# Releases and in-app updates

OpenWrap updates itself from GitHub Releases. Everything here is free: the
repository is public (so GitHub-hosted macOS runners and Releases cost
nothing) and no Apple Developer account is needed.

## How it works

1. `scripts/release.sh <version> "notes"` bumps the workspace version in
   `Cargo.toml`, commits, creates the annotated tag `v<version>` and pushes.
2. `.github/workflows/release.yml` runs on the tag: tests, `tauri build`,
   then `scripts/package-release.sh`, which
   - re-signs the app ad-hoc **without entitlements** (see the script header),
   - builds `OpenWrap-Share.zip` for first-time installs,
   - builds `OpenWrap.app.tar.gz` **after** re-signing and signs it with the
     updater key (`.sig`),
   - writes `latest.json`.
   It then publishes a GitHub Release with those four files.
3. On startup the app fetches
   `https://github.com/vinicius741/openwrap/releases/latest/download/latest.json`
   (`plugins.updater` in `src-tauri/tauri.conf.json`). When the version is
   newer it shows a banner; "Update and restart" downloads the archive,
   verifies its signature, swaps the app bundle (macOS asks for a password if
   the app folder is not writable), disconnects the VPN and restarts.
   Settings → Updates has a manual "Check for updates" button.

Files downloaded by the app itself do not get the macOS quarantine flag, so
updated builds open without Gatekeeper prompts.

## Updater signing key

The updater key is a free minisign key pair, unrelated to Apple code signing.

- Private key: `~/.tauri/openwrap-updater.key` on the maintainer's Mac, and
  the `TAURI_SIGNING_PRIVATE_KEY` repository secret (no password).
- Public key: `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.

**Back up the private key.** If it is lost, installed copies can no longer
accept updates; colleagues would have to reinstall manually once from a build
with a new public key.

To create a new key (only if the old one is lost):

```bash
cargo tauri signer generate -w ~/.tauri/openwrap-updater.key --ci
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/openwrap-updater.key
```

then copy `~/.tauri/openwrap-updater.key.pub` into `tauri.conf.json`.

## The privileged helper is not auto-updated

The setuid helper in `/Library/PrivilegedHelperTools` needs `sudo`, so an
update does not replace it. The app compares the installed helper with the one
bundled in the running app (`get_app_info`) and, when they differ, shows a
banner with the one-line `sudo install …` command to copy into Terminal.

## Local builds

`scripts/build-share-package.sh` builds and packages locally into
`~/Desktop/OpenWrap-Release/` (update files only when the key exists). Useful
for testing; normal releases go through `scripts/release.sh`.
