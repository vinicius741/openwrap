# Refactoring Priorities

This document identifies the files most likely to become maintenance problems if they keep growing.

It is based on three signals:

- File size.
- Responsibility count inside a single file.
- Change risk, especially where platform logic, orchestration, persistence, and UI state are mixed together.

The goal is not to split files for the sake of splitting them. The goal is to separate unstable responsibilities so future changes stay local.

## Priority Order

Start in this order:

1. `crates/openwrap-core/src/connection/manager.rs`
2. `crates/openwrap-core/src/dns/macos.rs`
3. `crates/openwrap-helper/src/main.rs`
4. `crates/openwrap-core/src/profiles/import.rs`
5. `ui/src/store/appStore.ts`
6. `crates/openwrap-core/src/storage/sqlite.rs`
7. `crates/openwrap-core/src/logging/mod.rs`
8. `src-tauri/src/tray/mod.rs`
9. `src-tauri/src/commands/profiles.rs`
10. `crates/openwrap-core/src/profiles/model.rs`
11. `ui/src/features/profiles/ProfileDetail.tsx`
12. `ui/src/features/connection/ConnectionPanel.tsx`

## Highest Priority

### `crates/openwrap-core/src/connection/manager.rs` (`2122` lines)

Why this is urgent:

- It is the largest file in the repo by a wide margin.
- It mixes public API, state machine transitions, credential flow, runtime file preparation, backend process orchestration, log parsing reactions, DNS observation updates, retry scheduling, cleanup, error mapping, and a large test module.
- The file is effectively both an application service and a bag of internal subsystems.

Suggested split:

- `connection/manager/mod.rs`
  Keep the public `ConnectionManager` API and re-exports here.
- `connection/manager/state.rs`
  `ManagerState`, `ActiveSession`, `PendingCredentials`, snapshot mutation helpers.
- `connection/manager/connect.rs`
  `connect`, `submit_credentials`, connect attempt orchestration.
- `connection/manager/runtime.rs`
  Runtime directory creation, auth file writing, launch config writing, cleanup helpers.
- `connection/manager/events.rs`
  Backend event loop, `handle_log`, `handle_exit`, retry scheduling.
- `connection/manager/errors.rs`
  Exit code mapping, terminal error helpers, DNS reconcile error handling.
- `connection/manager/tests.rs`
  Move the test module out of the main implementation file.

Refactor approach:

- First extract pure helpers without changing behavior.
- Then isolate state mutation behind small helper methods so fewer functions lock and mutate the store directly.
- Keep the external `ConnectionManager` surface stable while moving internals.

### `crates/openwrap-core/src/dns/macos.rs` (`1059` lines)

Why this is urgent:

- Most of the file is made of large embedded shell scripts.
- The scoped and global DNS paths duplicate a lot of shell logic.
- Rust orchestration and shell template content are coupled together, which makes both harder to review and test.

Suggested split:

- `dns/macos/mod.rs`
  Entry points such as `append_launch_config`.
- `dns/macos/paths.rs`
  Bridge directory and persistent-state path handling.
- `dns/macos/render.rs`
  Policy-specific renderers.
- `dns/macos/fragments.rs`
  Shared shell fragments reused by multiple renderers.
- `dns/macos/templates/*.sh`
  Move the large shell bodies into template files loaded with `include_str!`.

Refactor approach:

- Extract shared shell fragments before changing script behavior.
- Keep generated script output byte-for-byte compatible at first.
- Add focused tests around rendered output after extraction.

### `crates/openwrap-helper/src/main.rs` (`947` lines)

Why this is urgent:

- It mixes CLI entrypoints, request parsing, path validation, OpenVPN process launching, signal handling, event streaming, DNS reconciliation, process cleanup, shell command wrappers, parsing helpers, and tests.
- The file acts as both the helper program entrypoint and the full macOS repair engine.

Suggested split:

- `openwrap-helper/src/main.rs`
  Keep only argument dispatch and exit code wiring.
- `openwrap-helper/src/connect.rs`
  `run_connect`, child process launch, stdout/stderr forwarding, signal handling.
- `openwrap-helper/src/request.rs`
  JSON reading and request validation.
- `openwrap-helper/src/reconcile/mod.rs`
  Top-level DNS reconciliation flow.
- `openwrap-helper/src/reconcile/dns.rs`
  Global override, scoped resolver, route reconciliation.
- `openwrap-helper/src/reconcile/processes.rs`
  Runtime process inspection and orphan cleanup.
- `openwrap-helper/src/system.rs`
  Small wrappers around `networksetup`, `route`, `ps`, and cache flushing.
- `openwrap-helper/src/tests.rs`
  Move the large test module out of `main.rs`.

Refactor approach:

- Split by command first: `connect` vs `reconcile-dns`.
- After that, isolate command execution behind a thin system layer so reconciliation logic stops depending on raw shell invocations everywhere.

## Medium Priority

### `crates/openwrap-core/src/profiles/import.rs` (`669` lines)

Why this matters:

- One method handles validation, warnings, asset resolution, inline extraction, rewrite planning, report assembly, and persistence.
- Import rules are likely to grow as more profile edge cases are supported.

Suggested split:

- `profiles/import/mod.rs`
- `profiles/import/validator.rs`
- `profiles/import/assets.rs`
- `profiles/import/report.rs`
- `profiles/import/importer.rs`
- `profiles/import/tests.rs`

Refactor approach:

- Separate pure validation from filesystem work.
- Make asset copying and asset hashing its own pipeline.
- Keep `ProfileImporter` as a thin coordinator.

### `ui/src/store/appStore.ts` (`381` lines)

Why this matters:

- It is the central state container for profiles, connection state, logs, credentials, settings, detection, import warnings, and global errors.
- It mixes state definition, async effects, mutation logic, optimistic UI coordination, and cross-feature workflows.
- This file will keep growing because every new UI behavior has an easy path into the same store.

Suggested split:

- `ui/src/store/appStore.ts`
  Keep store assembly only.
- `ui/src/store/createAppStore.ts`
- `ui/src/store/slices/profileSlice.ts`
- `ui/src/store/slices/connectionSlice.ts`
- `ui/src/store/slices/settingsSlice.ts`
- `ui/src/store/slices/importSlice.ts`
- `ui/src/store/reducers/connectionEvents.ts`
- `ui/src/store/actions/loadInitial.ts`
- `ui/src/store/actions/profileActions.ts`
- `ui/src/store/actions/connectionActions.ts`

Refactor approach:

- Start by moving async workflows out of the store declaration.
- Keep the existing store API stable so components do not need a large rewrite immediately.
- Add selectors per slice before moving components to smaller hooks.
- Split `ui/src/types/ipc.ts` by domain only when the store slices start to form, so the new store modules are not still coupled through one shared contract dump.

### `crates/openwrap-core/src/storage/sqlite.rs` (`503` lines)

Why this matters:

- It mixes migrations, repository operations, raw SQL, row mapping, enum serialization rules, and tests.
- Persistence changes will become expensive because every concern lives in the same module.

Suggested split:

- `storage/sqlite/mod.rs`
- `storage/sqlite/schema.rs`
- `storage/sqlite/profile_queries.rs`
- `storage/sqlite/settings_queries.rs`
- `storage/sqlite/mappers.rs`
- `storage/sqlite/codec.rs`
- `storage/sqlite/tests.rs`

Refactor approach:

- Move row mapping and enum codecs first.
- Wrap multi-step writes like `save_import` and `delete_profile` in transaction helpers before splitting query modules.

### `crates/openwrap-core/src/logging/mod.rs` (`637` lines)

Why this matters:

- It combines domain models, active session lifecycle, disk layout concerns, retention cleanup, a thread-safe wrapper, and tests.
- The boundaries are already visible, which makes it a good candidate for a low-risk split.

Suggested split:

- `logging/mod.rs`
- `logging/model.rs`
- `logging/session_manager.rs`
- `logging/catalog.rs`
- `logging/shared.rs`
- `logging/tests.rs`

Refactor approach:

- Keep `writer.rs` as the low-level sink.
- Move session discovery and retention cleanup away from the active write path.

## Lower Priority But Worth Planning

### `src-tauri/src/tray/mod.rs` (`171` lines)

Why this matters:

- It mixes tray construction, menu event routing, connection actions, profile resolution, label/tooltip state derivation, and window management.
- The file is not huge yet, but it has several different reasons to change.

Suggested split:

- `src-tauri/src/tray/mod.rs`
  Public tray setup entrypoint.
- `src-tauri/src/tray/menu.rs`
  Menu construction and IDs.
- `src-tauri/src/tray/presenter.rs`
  Tooltip and enable/disable derivation from connection state.
- `src-tauri/src/tray/actions.rs`
  Connect, disconnect, and show-window handlers.
- `src-tauri/src/tray/target.rs`
  Resolve the selected or fallback profile target.

### `src-tauri/src/commands/profiles.rs` (`164` lines)

Why this matters:

- It mixes DTO parsing, command wiring, profile CRUD, filesystem cleanup, selection updates, and tray synchronization.
- Delete flow is already orchestration-heavy and will likely keep growing.

Suggested split:

- `src-tauri/src/commands/profiles.rs`
  Command registration only.
- `src-tauri/src/commands/profiles/import.rs`
- `src-tauri/src/commands/profiles/selection.rs`
- `src-tauri/src/commands/profiles/delete.rs`
- `src-tauri/src/commands/profiles/dns_policy.rs`
- `src-tauri/src/commands/profiles/parse.rs`
  Shared `ProfileId` parsing helper.

### `src-tauri/src/lib.rs` (`88` lines)

Why this matters:

- It is still small, but it is becoming the startup choke point for builder setup, app-state construction, connection-event forwarding, tray setup, and command registration.
- This file should stay as assembly, not become the place where more lifecycle logic accumulates.

Suggested split:

- `src-tauri/src/lib.rs`
  Thin assembly only.
- `src-tauri/src/bootstrap.rs`
  Builder and setup wiring.
- `src-tauri/src/event_forwarder.rs`
  Forward `CoreEvent` messages into Tauri events and tray synchronization.
- `src-tauri/src/invoke.rs`
  Centralize the invoke registry.

### `src-tauri/src/app_state.rs` (`85` lines)

Why this matters:

- It mixes dependency construction, backend selection, startup DNS reconciliation, and helper path resolution.
- It is not large today, but it is exactly the kind of file that quietly turns into setup glue nobody wants to touch.

Suggested split:

- `src-tauri/src/app_state/mod.rs`
- `src-tauri/src/app_state/container.rs`
  Dependency graph construction.
- `src-tauri/src/app_state/backend_factory.rs`
  Backend selection and helper resolution.
- `src-tauri/src/app_state/startup.rs`
  Startup reconciliation tasks.

### `crates/openwrap-core/src/profiles/model.rs` (`488` lines)

Why this matters:

- It is becoming a catch-all for IDs, assets, validation DTOs, parsed profile structures, import data, and runtime views.
- This is less urgent than the orchestration files, but the model layer will become noisy if every profile concept lands here.

Suggested split:

- `profiles/ids.rs`
- `profiles/assets.rs`
- `profiles/validation.rs`
- `profiles/profile.rs`
- `profiles/parsed.rs`
- `profiles/runtime.rs`

Refactor approach:

- Keep `profiles/mod.rs` as a re-export layer so callers do not need to update all imports at once.

### `ui/src/features/profiles/ProfileDetail.tsx` (`188` lines)

Why this matters:

- It mixes page composition, delete modal behavior, DNS policy editing, validation rendering, asset rendering, and log pane composition.
- It is still manageable today, but it is already acting as the feature shell for everything profile-specific.

Suggested split:

- `ui/src/features/profiles/ProfileDetail.tsx`
  Page composition only.
- `ui/src/features/profiles/components/ProfileHeader.tsx`
- `ui/src/features/profiles/components/DeleteProfileDialog.tsx`
- `ui/src/features/profiles/components/DnsPolicyCard.tsx`
- `ui/src/features/profiles/components/ValidationFindingsCard.tsx`
- `ui/src/features/profiles/components/ManagedAssetsCard.tsx`
- `ui/src/features/profiles/hooks/useDeleteProfile.ts`

### `ui/src/features/connection/ConnectionPanel.tsx` (`192` lines)

Why this matters:

- It mixes connection summary, control actions, DNS status interpretation, error presentation, log navigation, and credential form state.
- The file will likely grow once more connection states and diagnostics are added.

Suggested split:

- `ui/src/features/connection/ConnectionPanel.tsx`
  High-level composition only.
- `ui/src/features/connection/components/ConnectionSummary.tsx`
- `ui/src/features/connection/components/ConnectionControls.tsx`
- `ui/src/features/connection/components/ConnectionMetadata.tsx`
- `ui/src/features/connection/components/DnsStatusNotice.tsx`
- `ui/src/features/connection/components/ConnectionErrorBanner.tsx`
- `ui/src/features/connection/components/CredentialPromptForm.tsx`
- `ui/src/features/connection/model/status.ts`
  `isConnected` and DNS status message helpers.

## Files That Should Not Lead The Refactor

These files can be improved later, but they are not the best first targets:

- `ui/src/types/ipc.ts`
  This is more of a drift-risk file than a size problem. If it becomes painful, solve it with type generation or a shared contract layer, not by manually slicing it.

## Recommended Execution Strategy

If this refactor work starts later, use this sequence:

1. Extract test modules from the largest files where possible.
2. Split pure helpers from stateful orchestration.
3. Introduce new submodules behind the existing public APIs.
4. Only after that, move domain models and UI composition components.

Avoid this sequence:

- Starting with `profiles/model.rs` because it looks tidy to split.
- Renaming or relocating everything in one pass.
- Changing public imports and internal behavior in the same PR.

## Short Version

The main maintainability risk is not just large files. It is large files that also combine unrelated responsibilities.

The three files that deserve immediate attention when refactoring actually starts are:

1. `crates/openwrap-core/src/connection/manager.rs`
2. `crates/openwrap-core/src/dns/macos.rs`
3. `crates/openwrap-helper/src/main.rs`

After that, the next best return comes from:

1. `crates/openwrap-core/src/profiles/import.rs`
2. `ui/src/store/appStore.ts`
3. `crates/openwrap-core/src/storage/sqlite.rs`
