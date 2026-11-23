# OpenWrap architecture and code-quality review (2026-11-22)

Status: **draft for adversarial review**. Nothing here has been implemented.

Scope: whole repository at `main` (9b71154) plus the Windows auto-update work
(PR #7). About 15k lines of hand-written code: `crates/openwrap-core` (~9k
incl. tests), `crates/openwrap-helper` (~1k), `src-tauri` (~3k Rust +
~1.8k Objective-C for the optional Packet Tunnel), `ui/` (~3k TS/TSX + CSS).

Baseline: 212 Rust tests pass (core + helper), 60 Node tests pass, UI builds.
CI runs macOS + Windows (with a real OpenVPN E2E on Windows).

Project constraints respected throughout (from `CLAUDE.md`/`AGENTS.md`): hobby
project, setuid helper is the intended macOS backend, Packet Tunnel stays
opt-in, Windows code stays behind `cfg(windows)`.

---

## 0. Executive summary (honest version)

The codebase is **better than "vibe-coded" suggests**. Layering
(UI → Tauri commands → core with traits) is sensible, the directive policy is an
allow-list, there is a scripted-backend test suite for the connection manager,
CI covers both shipping platforms, and modules were already split up in earlier
refactors. **There is no performance problem** anywhere; nothing proposed here
will make the app noticeably faster, and I am not claiming speed gains.

What *is* wrong falls into three buckets, in order of importance:

1. **Security (critical):** the setuid-root helper will run any executable,
   and any OpenVPN config/script, that the *calling user* points it at. Any
   process running as any local user can get root without a password.
2. **Connection reliability bugs (high, user-visible):** stale TOTP passwords on
   reconnect, lost final OpenVPN lines on macOS, a "stuck state" that needs an
   app restart, a disconnect race, and config rewriting that breaks quoted args.
3. **Structural causes of (2) (medium):** the connection manager's state is
   mutated in ~10 places by free functions with 8–11 parameters, bypassing the
   state machine; process supervision is copy-pasted between the macOS and
   Windows launchers so fixes don't propagate; OpenVPN state is inferred by
   string-matching log lines.

Everything else (UI store size, hand-written IPC types, test hygiene, CI lint)
is real but low-impact and should not drive the decision.

---

## 1. Findings

Severity: **Critical / High / Medium / Low**. "Verified" = I traced it in code;
"Plausible" = depends on OpenVPN/server behavior I could not run here.

### A. Security

#### A1 — Setuid helper is a local privilege escalation to root (Critical, verified)

`/Library/PrivilegedHelperTools/app.openwrap.desktop.openwrap-helper` is
installed `root:wheel 4755` (world-executable). `openwrap-helper connect` reads
a JSON `ConnectRequest` from stdin and:

- `validate_openvpn_binary` (`crates/openwrap-helper/src/request.rs`) only
  checks "absolute path, regular file, has an x bit". **Any executable** is then
  spawned with euid 0 (`connect.rs`). A user-owned compiled binary can
  `setuid(0)` and is full root. The user-facing `openvpn_path_override` setting
  is literally this feature.
- `validate_config_path`/`validate_scoped_path` only check the config, runtime
  dir and auth file are under `~/Library/Application Support/OpenWrap/…` of
  the **invoking** user. Those directories are **user-writable**, so the check
  constrains nothing: write a config with `script-security 2` + `up /path/x`
  and OpenVPN runs it as root. The directive allow-list in
  `config/policy.rs` is only enforced at *import* time inside the
  unprivileged app; the helper never re-checks.
- The app itself relies on this hole: macOS DNS scripts are rendered by the
  unprivileged app into `$TMPDIR/openwrap-dns/<session>/` (user-writable,
  `dns/macos/paths.rs::bridge_dir`) and wired in via `route-up`/`down`. A
  process running as the user can swap the script between write and exec.
- `reconcile-dns` reads `*.tsv` state from the user-writable runtime dir and
  feeds it to `networksetup`/`route` as root (lower impact: DNS/route
  tampering, not code exec, since args go through argv not a shell).
- Because validation uses the *caller's* home, **other local accounts** can
  use the helper the same way.

`docs/security-model.md` says the helper "validates app-managed paths"; that is
true but gives no protection, so the doc overstates the guarantee.

Threat model, honestly: on a single-user Mac, malware already running as you
can steal a lot without root. Root adds silent persistence, system-wide
traffic/DNS control, disabling security tooling, etc., and Apple treats
user→root as a boundary. A world-executable setuid binary that execs
caller-chosen programs is a textbook CVE.

**Fix (stays within the free helper design):**
1. Helper owns the trust decision. It accepts a *structured* request
   (profile id, session id, DNS policy + match domains, credentials over
   stdin) instead of file paths it trusts.
2. Helper reads the managed profile, re-runs the **same core allow-list**
   (`openwrap-core` is already a dependency) and rejects anything not allowed
   (no scripts, plugins, management, log paths, `config` includes, etc.).
3. Helper writes the final config, asset copies, auth data and *its own*
   rendered DNS scripts into a **root-owned 0700** directory
   (e.g. `/var/run/openwrap/<uid>/<session>/`), and keeps DNS reconcile state
   there too. Nothing that root executes or trusts lives in a user-writable
   place.
4. OpenVPN binary: only from a fixed allow-list of install locations; drop the
   free-form override on macOS (or require root ownership for overrides).
5. Optional hardening: restrict helper execution to the `admin` group
   (`root:admin 4750`) as defence in depth for other accounts.

**Residual risk to state plainly:** Homebrew on Apple Silicon (`/opt/homebrew`)
is owned by the user, so the user can replace `openvpn` *or its dylibs*
(openssl, lzo, lz4, pkcs11-helper) and still get root through any
"run Homebrew OpenVPN as root" design. Closing that fully needs a root-owned
OpenVPN (e.g. Setup copies a statically linked OpenVPN, built in CI, into
`/Library/PrivilegedHelperTools/`). I list that as **A1b** (optional, bigger).
Even without A1b, steps 1–4 remove the trivial, no-skill exploit paths (point
at any binary / edit a config / swap a script) and leave only "tamper with
Homebrew's files", which is the same exposure every Homebrew user already has
when they `sudo` a Homebrew tool.

Expected outcome: closes the critical hole (except the Homebrew-tamper residual
unless A1b is done). No user-visible behavior change except that a custom
OpenVPN path on macOS must be in an allowed location. Requires users to
reinstall the helper once (the app already detects helper version mismatch
and offers "Install helper").

#### A2 — "Logs are sanitized" is mostly cosmetic (Low, verified)

`sanitize_log` replaces the literal *word* `password`/`PASSWORD`, not secret
values; raw lines are written unsanitized to `openvpn.log`. OpenVPN at verb 3
does not print credentials, so actual exposure is low. Fix: either make the
sanitizer meaningful (redact known secret values: username/password/OTP of the
current session) or stop claiming it in docs.

#### A3 — PIN + TOTP seed stored in a plaintext SQLite file (Low, documented)

`openwrap-secrets.sqlite3` (0600). It's documented. Likely chosen because
ad-hoc-signed builds lose Keychain ACLs on every update. Not proposing a change
unless reviewers think Keychain is workable; mention only.

### B. Connection reliability (user-visible bugs)

#### B1 — PIN+TOTP profiles reuse a stale one-time password (High, verified in code / server-dependent impact)

`connect()` generates `pin+otp` once and stores it in `ConnectionPlan.password`
(`manager/state.rs`). Every app-level retry (`schedule_retry` → 2 s, 5 s, 10 s)
reuses it, and the same value sits in `auth.txt` for the whole session. The
helper passes `--auth-nocache`, so OpenVPN **re-reads `auth.txt` whenever it
re-authenticates**: soft restarts (`SIGUSR1`, ping-restart after sleep/wake or
Wi-Fi change) and TLS renegotiation (`reneg-sec`, default hourly) unless the
server pushes an `auth-token`. Result: the OTP is expired or already used →
`AUTH_FAILED` → terminal error telling the user to "re-enter your username and
password" (which they never typed). PIN+TOTP is the feature this project
seems to have been built around, so this probably affects the main user.

Fix: regenerate the password for every attempt, and refresh it for OpenVPN's
in-process re-auth (best done via the management interface, see C3; a smaller
fallback is to rewrite `auth.txt` just before each OTP window, which is ugly).

#### B2 — macOS (default) backend can drop OpenVPN's last lines (High, verified)

`HelperOpenVpnBackend` sends `BackendEvent::Exited` as soon as `wait()` returns,
without waiting for the stdout/stderr reader tasks. The manager's event loop
`break`s on `Exited`, so lines still in the pipe (`AUTH_FAILED`, the fatal
error) are dropped, or processed after the retry decision. Consequences:
retrying with bad credentials, generic "exited with status 1" instead of the
real diagnostic. The **exact same bug was fixed for Windows** in e0c1d7c
(drain with a 500 ms timeout) but not for macOS, because the launch code is
duplicated (see C2).

#### B3 — Stuck state after an early connect error needs an app restart (High, verified)

`ConnectionManager::connect` transitions to `ValidatingProfile`, then can return
early with `?` (secret store error such as a denied Keychain prompt, missing or
invalid PIN/TOTP config, TOTP generation error). No terminal state is set, so
the snapshot stays `validating_profile`. In the UI, `isConnected()` is false in
that state: **Disconnect is disabled and Connect fails** with "invalid
transition from ValidatingProfile with BeginConnect". The only way out is
restarting the app. Same pattern in `submit_credentials` (pending prompt taken,
then a Keychain/DB error leaves `awaiting_credentials` with no prompt).
Relatedly, the UI disables Disconnect during `preparing_runtime` /
`starting_process`, so a hung start cannot be cancelled.

#### B4 — The state machine is bypassed; disconnect races (Medium, verified)

`transition()` exists but `handle_log`/`handle_exit` assign
`snapshot.state = Connected | Reconnecting | Error` directly. Example: the user
clicks Disconnect, OpenVPN prints `Initialization Sequence Completed` while
shutting down. The state flips back to `Connected`. On exit it is no longer
`Disconnecting`, the reconnect plan was cleared, so the user gets a
"Connection failed" error for a disconnect they asked for. `RetryableFailure`
guards against `Disconnecting`, but `Connected` does not. These are symptoms of
C1.

#### B5 — Config rewrite drops quoting (Medium, verified)

`tokenize_line` strips quotes; `rewrite_profile` joins args with spaces. So
`verify-x509-name "VPN Server CN" name` becomes
`verify-x509-name VPN Server CN name` at launch, which OpenVPN rejects or
misreads. Any quoted argument with spaces breaks. Small fix: re-quote args
when writing.

#### B6 — Deleting a connected profile races the disconnect (Low, verified)

`delete_profile` calls `disconnect_if_connected`, which only *starts* the
disconnect, then removes the managed dir whose assets the still-running OpenVPN
references by absolute path. Mostly harmless, but it's the kind of ordering
issue the structural fix (C1) handles for free (await the Idle state).

### C. Architecture and maintainability

#### C1 — Connection manager: scattered mutation, 8–11-parameter free functions (Medium, verified)

`start_connect_attempt` (9 params), `handle_log` (11), `handle_exit` (8) and
`schedule_retry` (10) take clones of every dependency because `ConnectionManager`
isn't `Arc<Inner>`. Snapshot fields (`state`, `pid`, `substate`,
`last_error`, `log_file_path`, `retry_count`…) are hand-edited in ~10 places
across `state.rs`, `connect.rs`, `events.rs` and `errors.rs`, each slightly
different (B3, B4). Blocking work (helper spawn + wait for DNS reconcile,
file I/O, `persist_failed_connection_log` under the state mutex) runs on Tokio
worker threads.

Proposed shape: `ConnectionManager { inner: Arc<Inner> }` with methods; one
**actor task** per manager that owns `ManagerState` and processes a single
`enum Input { Connect, SubmitCredentials, Disconnect, Backend(session, gen, BackendEvent), RetryTimer(gen), Shutdown }`.
Every state change goes through one `reduce(state, input) -> effects` function
that uses the state machine and is unit-testable without Tokio. Blocking effects
run via `spawn_blocking`. This makes B3/B4-class bugs structurally hard and the
code much smaller.

Expected outcome: no speed change; fewer lifecycle bugs; manager code shrinks
(estimate −30–40% LOC in `connection/manager/`); tests can target the reducer
directly. Risk: this is the heart of the app; mitigated by keeping the existing
20 behavioral tests green and adding race tests.

#### C2 — Process supervision copy-pasted three times (Medium, verified)

Spawn + pipe readers + child map + kill + reap logic exists in
`helper_launcher.rs`, `direct_launcher.rs` and the helper's `connect.rs`, and
they have already diverged (B2). Extract one `ProcessSupervisor` used by both
backends (event ordering guaranteed: all output, then `Exited`).

#### C3 — OpenVPN state inferred by grepping log lines (Medium, design)

`Connected` = line contains `Initialization Sequence Completed`; retry =
`SIGUSR1`/`Restart pause`; auth failure = `AUTH_FAILED`. Credentials go
through an on-disk `auth.txt` for the session's whole lifetime. Disconnect is
`/bin/kill -TERM <helper pid>` from a spawned process.

OpenVPN's **management interface** (what Tunnelblick and openvpn-gui use)
provides structured `>STATE:` (CONNECTING/WAIT/AUTH/GET_CONFIG/ASSIGN_IP/
CONNECTED/RECONNECTING/EXITING), `>PASSWORD:Need 'Auth'` (so the app supplies
credentials **on demand**: fresh TOTP every time, no password file on disk),
`>FATAL:`, byte counts, and a graceful `signal SIGTERM`. It fixes B1
properly, removes the credential file, and replaces string-matching with a
protocol.

Security design: on macOS the helper creates a unix socket in its root-owned
session dir and proxies the management channel over its own stdin/stdout. The
app never touches the socket and the helper only forwards a small allow-list
of commands (`username`, `password`, `signal SIGTERM`, `state on`,
`hold release`). On Windows, a localhost TCP port with a random password
file, like openvpn-gui.

This is the **largest** item and the one with the most risk. It's also what
makes the B1 fix clean. Log lines are still forwarded for display and
diagnostics; `diagnose_exit_error` stays.

#### C4 — Business logic in the Tauri command layer (Low–Medium, verified)

Profile deletion orchestration, generated-password validation + rollback, and
PIN validation (duplicated in core `resolve_generated_password` and
`commands/profiles/generated_password.rs`) live in `src-tauri`. That crate
can't build on Linux and has little test coverage. Move to core services
(`ProfileService`), leaving commands as thin adapters.

#### C5 — Hand-maintained TypeScript mirror of Rust types (Low–Medium)

`ui/src/types/ipc.ts` duplicates ~25 Rust types by hand, with mixed
snake_case/camelCase conventions. Drift is caught only at runtime. Option:
generate with `ts-rs` (or `specta`) in a test that fails CI if the generated
file changes. Modest value; cheap.

#### C6 — Optional Packet Tunnel: ~2k lines nobody can build or run (Decision for maintainer)

`network_extension_backend.rs` + ObjC bridge + `OpenWrapPacketTunnelProvider.mm`
+ build scripts. Per project policy it is intentionally opt-in. I'm **not**
calling it incomplete; I'm noting that code the maintainer can't run without a
paid account will rot and taxes every refactor of `VpnBackend`/`ConnectRequest`.
Options: keep (status quo), or move it to a branch/tag. Maintainer's call; the
plan below works either way (keep it compiling).

#### C7 — Diagnostics go to `eprintln!` (Low)

A GUI app's stderr goes nowhere. Use `tracing` with a rolling file in the logs
dir (session logs stay as they are). Helps when "it didn't connect" happens on
a machine you can't see.

### D. Hygiene (Low; include only because it's cheap)

- CI doesn't run `openwrap-helper` tests, `cargo clippy -D warnings`, or
  `cargo fmt --check`. Clippy currently has one deny-level error (test code)
  and ~15 warnings.
- Low-value tests that assert struct field assignment
  (`session.rs`, `openvpn/backend.rs`) inflate counts. Node "tests" grep source
  files for strings (brittle; they test text, not behavior).
- Dead code: `connectSelectedError`/`disconnectError`/`submitCredentialsError`,
  unused `SERVICE_NAME`, unused `tauri-plugin-fs`, duplicate
  `quote_openvpn_arg`.
- UI: no linter, no unit tests. The 565-line Zustand store is fine for this app
  size. **Not worth restructuring.**

---

## 2. What I recommend NOT doing (to avoid marginal churn)

- Rewriting the UI/store, swapping state libraries, CSS restructuring.
- Switching Tauri/React patterns, adding a DI framework, "clean architecture"
  layers.
- Making Packet Tunnel the default (forbidden, and pointless without paid signing).
- Chasing performance: there is no measurable bottleneck.

---

## 3. Proposed plan if we commit (maximum-impact version)

Ordered so each phase is shippable and verifiable on CI (macOS + Windows).

1. **Phase 1, helper trust boundary (A1 steps 1–5).** Helper re-validates
   the profile with the core allow-list, renders DNS scripts itself, uses a
   root-owned session dir, allow-lists OpenVPN locations, and reconcile state
   moves to the root dir. Add helper tests (CI currently skips them).
2. **Phase 2, connection core rewrite (C1 + C2 + B2–B6).** Actor + reducer
   manager, shared process supervisor with correct event ordering, every
   failure path ends in a terminal state, Disconnect enabled in every
   non-idle state (cancel), quoting-preserving rewrite. Keep all 20 existing
   behavioral tests and add race tests.
3. **Phase 3, management interface (C3 + B1).** Structured state, on-demand
   credentials (fresh TOTP per auth, no `auth.txt`), graceful `signal SIGTERM`.
   Log-based detection stays as fallback/diagnostics during transition.
4. **Phase 4, supporting cleanups (C4, C5, C7, D, A2).** Core profile
   service, generated TS types, `tracing`, CI clippy/fmt/helper tests,
   dead-code removal, honest sanitizer/docs.
5. Optional **A1b**: root-owned, statically linked OpenVPN shipped by Setup.
   Optional **C6**: per maintainer decision.

## 4. Expected outcomes

| Area | Today | After | Confidence |
|---|---|---|---|
| Local root escalation | Trivial (any executable / any config) | Removed except Homebrew-file tampering (removed too with A1b) | High |
| PIN+TOTP reconnect after sleep/network change / hourly reneg | Likely ends in AUTH_FAILED (server-dependent) | Fresh OTP on every auth | Medium–High (server-dependent today) |
| Wrong/generic error after auth failure on macOS | Racy | Deterministic | High |
| Stuck state needing app restart | Reachable (e.g. denied Keychain prompt) | Not reachable; Disconnect always available | High |
| Disconnect reported as "Connection failed" | Rare race | Not possible (single reducer) | High |
| Profiles with quoted args (e.g. `verify-x509-name "A B"`) | Break at launch | Work | High |
| Password on disk during session | `auth.txt` for whole session | None (management interface) | High |
| Speed / resource use | Fine | Same (no claim) | — |
| Maintainability | Fixes don't propagate between launchers; 10-param functions | One supervisor, one reducer, helper self-contained | Medium–High |
| Cost / risk | — | Large change to the connection path and helper; users reinstall helper once; macOS-specific code can only be verified on CI runners, and real VPN behavior only by the maintainer | — |

## 5. My own verdict (before review)

**Worth doing: yes, but only because of A1 and B1–B4.** Those are concrete
security and reliability defects, not taste. C1–C3 are justified as *the way to
fix them properly* rather than for elegance. If reviewers show A1 is not
exploitable as described, or B1 is not real, the case gets much weaker and a
small targeted bug-fix PR (B2, B3, B5) would be the right call instead of a
rewrite.

---

## 6. Adversarial review results

### 6.1 Fable 5.2: verdict (b), reduced scope

"Do one small bug-fix PR in core (B2, B3, B4, B5 and a light B1) plus a tiny
helper-hardening + docs PR, then stop. Skip C1/C2/C3 and Phase 4." Claims I
re-verified in code are marked ✔.

Corrections accepted into this assessment:

- **A1 residual risk was wrong.** ✔ After Phase 1 steps 1–4 the exploit for the
  Homebrew-owning admin becomes "overwrite `/opt/homebrew/sbin/openvpn`, call
  the helper": one extra `cp`. My comparison to "sudo a Homebrew tool" was
  invalid: sudo asks for a password, the helper doesn't. So Phase 1 without
  A1b barely moves the needle for the main user. A1b (ship and maintain a
  root-owned OpenVPN + libraries) is the real fix and the expensive part.
  `root:admin 4750` (one line + the app's mode check at
  `src-tauri/src/commands/updates.rs:250`, ✔) cheaply closes the
  "other local accounts" part.
- **B1 severity inflated (High → Medium).** `--auth-nocache` isn't the cause
  (without it OpenVPN would resend the cached stale OTP anyway); hourly reneg
  failing would break every client, so real servers push `auth-token`. Real
  exposure: retries/restarts long after the OTP was issued, plus a misleading
  "re-enter your username and password" error. New point: even a fresh OTP
  fails within the same 30 s step if the server rejects replays, so a retry
  should wait for the next step. The management interface doesn't solve that
  either.
- **B2 Medium, not High** (intermittent). Fix is ~10 lines (port the Windows
  drain).
- **B3 confirmed for `connect`, wrong for `submit_credentials`** (that state
  keeps Disconnect enabled). The backend `disconnect()` already handles
  `ValidatingProfile`; the dead end is UI and **tray** gating ✔
  (`src-tauri/src/tray/presenter.rs:74`). More sites of the same class:
  `session_log.start_session(..)?` and the `Spawned` transitions in
  `connect.rs`.
- **C1 remedy unjustified:** B3/B4 are fixable in ~40 lines; the actor rewrite
  is the "marginal refactor" the maintainer wants to avoid. **C2:** only two
  launchers really diverge; port the fix, don't abstract. **C3:**
  over-engineered and counterproductive: it adds a protocol proxy to the
  setuid binary that A1 says is already too trusting.
- **Missed issue (new):** orphaned root OpenVPN race. If Disconnect lands while
  `start_connect_attempt` is between spawn and the `Spawned` transition, the
  transition `?` fails after `active_session` is set and before the event loop
  starts. OpenVPN keeps running while the UI shows Idle. Fix: kill the
  session and set a terminal error on that path.
- **Missed issue (new):** `reconcile-dns` as root lets any local user change
  DNS on any service, delete files containing the marker strings, and SIGTERM
  processes that mimic a managed OpenVPN command line. Low; mostly closed by
  4750.
- "Verifiable on CI" is weak for setuid/root-dir/real-tunnel behavior.

### 6.2 Gemini 4 Neon: verdict (b), reduced scope

"Do not run the 4-phase rewrite." Gemini agrees with Fable on A1's
residual-risk error, on B1–B5 being real and small, on C1/C2/C3 being
unjustified, and that CI should run helper tests first. Additional points
accepted (✔ = re-verified in code):

- **C1 actor design flaw:** `ConnectionManager::shutdown()` runs synchronously
  from macOS `application_will_terminate` with no Tokio runtime, so an actor
  would need shared mutex state anyway. One more reason to drop C1.
- **B1 cheap fix:** regenerate the TOTP password per attempt, and add
  `remap-usr1 SIGTERM` for PIN+TOTP profiles so OpenVPN's soft restarts become
  process restarts that get a fresh OTP, with no management interface.
- **Missed: macOS shutdown races DNS teardown.** ✔ `HelperOpenVpnBackend`
  doesn't override `disconnect_on_shutdown`, so quitting sends SIGTERM and
  immediately deletes the runtime dir and DNS `down` scripts while OpenVPN is
  still exiting, then reconciles DNS concurrently. Windows waits; macOS doesn't.
- **Missed: same orphaned-OpenVPN race as Fable**, plus another B3 site
  (`start_session(..)?`).
- **Missed: import symlink escape.** ✔ `resolve_asset_path` checks the
  *parent* is inside the import dir, then returns the canonicalized *target*
  without re-checking, so a symlinked `ca`/`cert` can pull in any file the
  user can read.
- **Missed: root file write in reconcile.** ✔ `reconcile_global_override`
  does `fs::write` as root to `global.tsv` in a user-writable dir (follows
  symlinks), and `reconcile_scoped_resolvers` deletes paths read from
  `scoped.tsv` with no `/etc/resolver/` prefix check (it does require an
  OpenWrap marker in the file).
- **Missed: IPC drift.** ✔ `LogEntry.level` is `'Debug'|'Info'|…` in TS, but
  Rust sends snake_case (masked by `.toLowerCase()`). Unused `fs:default`
  capability and `tauri-plugin-fs`.
- **Corrections to my D list:** `SERVICE_NAME` *is* used on macOS/Windows
  (clippy on Linux misled me). `dns-domains.mjs` and `runtime-installer.mjs`
  are real behavior tests, not source greps.

### 6.3 Additional finding from re-checking during review

- **Retry budget never refills.** ✔ `snapshot.retry_count` resets only on a
  fresh user connect (`connect.rs`), never on reaching `Connected`. A long
  session that drops three times (e.g. three sleep/wake cycles in a day) gets
  no automatic reconnect on the fourth drop.

---

## 7. Decision

Both reviewers independently answered **"yes, but reduced scope"**. The
big structural rewrite (C1 actor/reducer, C2 supervisor abstraction, C3
management interface, A1b bundled OpenVPN, C4/C5/C7) is **not worth it**: high
macOS-only regression risk, and the bugs it was meant to fix can each be fixed
directly in a few dozen lines. That part is dropped.

The targeted fixes **are** worth it, and we commit to the full set both
reviewers support:

**PR: connection reliability (core + UI + tray)**
1. B3: every early-error path in `connect()` / `start_connect_attempt` /
   `submit_credentials` ends in a terminal `Error` state (incl.
   `start_session` and the `Spawned` transitions).
2. Orphaned-OpenVPN race: if the session was cancelled while spawning, kill
   the new process and clean up instead of leaking it.
3. Disconnect (cancel) enabled in every non-idle state in the UI and tray.
4. B4: a late "Initialization Sequence Completed" can't override
   `Disconnecting`.
5. B2: helper backend drains OpenVPN output before reporting exit (port of the
   Windows fix).
6. macOS shutdown waits (bounded) for the helper/OpenVPN to exit, so DNS
   `down` scripts run before cleanup/reconcile.
7. B1: PIN+TOTP password generated per attempt; retries never reuse an OTP
   step; `remap-usr1 SIGTERM` for PIN+TOTP profiles; accurate error text.
8. Retry budget refills after a successful connection.
9. B5: `rewrite_profile` re-quotes arguments.
10. Import symlink escape closed.
11. IPC `LogEntry.level` type fixed; unused fs plugin and capability removed;
    dead UI helpers and the duplicate `quote_openvpn_arg` removed; clippy deny
    error fixed.

**PR: helper hardening + honest docs**
1. Helper refuses callers that aren't the logged-in console user (closes
   other-account / SSH / background-account use, without breaking standard
   users the way `root:admin 4750` would).
2. Helper only launches OpenVPN from known install locations.
3. Reconcile: refuse symlinked state files; only delete resolvers under
   `/etc/resolver/`.
4. CI runs `openwrap-helper` tests.
5. `docs/security-model.md` states plainly what the helper does and does not
   protect against (passwordless root for the primary user via
   user-owned Homebrew remains, by design of the free path).

Expected outcome: the user-visible reliability rows of the §4 table hold
(stuck state, disconnect race, quoting, macOS lost diagnostics, PIN+TOTP
reconnects, retry budget, clean DNS teardown on quit). The security row is
corrected to "other local accounts can no longer use the helper; the
primary user's passwordless-root exposure through user-owned Homebrew
remains and is documented". No speed claims.
