# Prompt: finish and validate Windows support (handoff)

> For a contributor with a Windows PC. The maintainer started Windows support on
> a Linux machine without Windows, so nothing has run on real Windows yet. Paste
> the block below into Claude Code / Codex running on the Windows machine, from
> an empty folder (the agent clones the repo itself).

---

You are finishing **Windows support** for OpenWrap, a Tauri 2 desktop client for OpenVPN profiles (React in `ui/`, Rust in `src-tauri/` and `crates/`). The repository is `https://github.com/vinicius741/openwrap`, and it belongs to another person (the maintainer). I am a contributor using **my own GitHub account**, which cannot push to that repository. You are running on my Windows PC and can build, run, and test everything here for real.

The code is already written. It lives on branch **`feat/windows-support`**, with draft PR **https://github.com/vinicius741/openwrap/pull/5**. It was only compile-checked (cross-compiled from Linux), never run on Windows. Your job is to run it, find and fix what breaks, and hand the result back as a pull request.

## 1. Git and PR workflow (my account, not the maintainer's)

1. Check `gh auth status`. If I am not logged in, ask me to run `gh auth login`. Make sure `git config user.name` / `user.email` are mine. Never commit as the maintainer.
2. Fork and clone, then base your work on the maintainer's branch:
   ```powershell
   gh repo fork vinicius741/openwrap --clone --remote
   cd openwrap
   git fetch upstream feat/windows-support
   git switch -c windows-validation upstream/feat/windows-support
   ```
3. Make small, focused commits (one fix per commit, conventional prefixes such as `fix(windows): ...`). Push to my fork: `git push -u origin windows-validation`.
4. Open the PR **against the maintainer's feature branch**, not `main`:
   ```powershell
   gh pr create --repo vinicius741/openwrap --base feat/windows-support --head <my-github-user>:windows-validation --draft
   ```
   In the body, reference PR #5 ("Follow-up to #5") and include the validation report from section 6. The maintainer merges it into `feat/windows-support`, and PR #5 picks up the commits.
5. Push as you go so the maintainer can see progress. Do **not** push to `main` or close or modify PR #5 itself.

## 2. Read first

`CLAUDE.md`, `AGENTS.md`, `docs/windows-setup.md` (describes the intended Windows design), `docs/security-model.md`. Then inspect the Windows code: `git diff main...HEAD`, especially:
- `crates/openwrap-core/src/windows.rs`: Win32 wrappers (elevation check, named exit event, process handle, registry lookup)
- `crates/openwrap-core/src/openvpn/direct_launcher.rs`: launch with `--service <event> 0`, `CREATE_NO_WINDOW`, graceful disconnect
- `crates/openwrap-core/src/dns/windows.rs`, `dns/observer.rs`: DNS policy mapping
- `crates/openwrap-core/src/secrets/keychain.rs`: Windows Credential Manager store
- `src-tauri/build.rs`, `src-tauri/windows/openwrap.exe.manifest`, `src-tauri/tauri.windows.conf.json`
- `src-tauri/src/commands/{reveal.rs,runtime.rs}`

## 3. Rules that cannot be broken

- **macOS behavior must not change.** Windows code stays behind `#[cfg(windows)]`. Do not remove or alter the setuid helper or the Packet Tunnel backend. The macOS CI job must stay green. Avoid touching shared code. If you must, keep macOS behavior byte-for-byte identical and explain why in the commit.
- **Hobby project, no costs:** no paid code-signing certificate. An unsigned build with a SmartScreen warning is fine.
- **Security model:** imported `.ovpn` files are untrusted (the allowlist in `crates/openwrap-core/src/config/policy.rs` still applies). Passwords are **never** persisted, only usernames. The temporary auth file must be deleted after use. Logs sent to the UI are sanitized.
- Scope: make the **run-as-administrator** design work well. Do **not** implement the Interactive Service / management-interface backend or Windows release packaging (`release.yml`, `latest.json`) unless I explicitly ask. List them as follow-ups instead.
- Follow repo conventions (file naming, style, comment density). Run `cargo fmt` only on files you touch.

## 4. Prerequisites on this machine (check, and tell me what's missing)

- Rust stable with MSVC (Visual Studio Build Tools, "Desktop development with C++"), Node 22, `cargo install tauri-cli --version "^2.0.0" --locked`, WebView2.
- OpenVPN Community 2.6+ (`winget install --id OpenVPNTechnologies.OpenVPN --exact`), installed with default options.
- A working `.ovpn` test profile. **Ask me for one.** Never commit it, and never paste credentials into logs, commits, or the PR.
- An **elevated** PowerShell (Run as administrator). The app manifest requires elevation, so `npm run tauri:dev` from a normal terminal fails with os error 740.

## 5. Validation checklist (fix what fails, re-test, and record evidence)

Build and tests:
1. `npm install`, `npm test`, `cargo test -p openwrap-core -p openwrap-app` all pass. Also check whether `cargo test` works from a **non-elevated** terminal. If the admin manifest leaks into test binaries (error 740), fix `build.rs` so it applies only to the app binary.
2. `npm run tauri:dev` opens the window (from an elevated terminal). `npx @tauri-apps/cli@2 build --bundles nsis` produces an installer.

Connection (the important part):
3. Import a `.ovpn` file and connect. The state reaches **Connected**, and OpenVPN logs appear live in the UI. **Verify stdout logs still arrive when OpenVPN runs with `--service`.** If not, report the cause and propose a fix (for example `--log` to a file that is tailed).
4. Graceful disconnect. Capture `route print -4`, `Get-DnsClientServerAddress`, and `Get-DnsClientNrptRule` **before connecting, while connected, and after disconnecting**. Routes and DNS must return to the "before" state. Test disconnect from the UI, from the tray, and by quitting the app mid-connection. Then connect again. Confirm OpenVPN logs a clean exit (not a forced kill) and measure how long it takes. If `--service` does not trigger a clean shutdown, check how openvpn-gui stops OpenVPN (its source is on GitHub, `OpenVPN/openvpn-gui`) and fix it.
5. Kill OpenWrap from Task Manager while connected. Observe and document what happens to `openvpn.exe`, routes, and DNS (known limitation, no fix required unless simple).
6. Wrong credentials show the proper error. "Remember username" survives an app restart (check `cmdkey /list | findstr openwrap`), and deleting the profile removes the entry.
7. DNS policies (profile setting): Split (default), Full override (adds `block-outside-dns`), Observe only (pushed DNS ignored). Check that each behaves as `docs/windows-setup.md` describes.
8. A Windows user whose profile path contains a **space** (create a test local user if needed). The generated config in `%LOCALAPPDATA%\OpenWrap\runtime\...` must load its `ca`/`cert`/`key` files (paths are escaped by `quote_openvpn_arg`).

App features:
9. OpenVPN detection (registry and Program Files) and the custom path override in Settings.
10. "Install OpenVPN" (winget) on a machine without OpenVPN, if feasible, or at least the clear error when winget is missing.
11. "Reveal log file" / reveal profile / logs folder open Explorer with the item selected.
12. NSIS installer: install, launch (UAC prompt expected), check the "Run OpenWrap" checkbox at the end of setup (requireAdministrator can break it), uninstall.
13. Look for any remaining macOS-only wording in the UI ("Finder", "Keychain", "Terminal", "Mac password") and make it platform-aware with `ui/src/lib/platform.ts`.

## 6. Hand back

- The PR from section 1, containing a **validation report**: one row per checklist item (pass / fixed / limitation), the commands you ran, and relevant output (trimmed, and with no IPs, hostnames, usernames, or credentials from my VPN; replace them with placeholders).
- Update `docs/windows-setup.md`: remove "experimental / not validated" wording that no longer applies and adjust "Known limitations".
- Stop and ask me before anything outside this scope, and whenever a decision changes the design (for example dropping the admin requirement).
