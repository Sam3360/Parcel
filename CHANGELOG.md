# Changelog

## 1.1.0 — unreleased

Backward-compatible release: **package format stays 1.0 and the database schema stays v1.** Packages written by 1.1.0 open in 1.0.0 and vice versa; an existing database needs no migration.

### Fixed
- **Dependency checks disappeared when any source file was missing.** Diagnostics reused export preparation, which refuses to run with missing sources, and the UI swallowed the error. Diagnostics are now a separate static analysis that always runs.
- **Permanently deleting an imported workspace leaked its extracted files forever.** They are now removed once nothing references them (duplicated workspaces share files; trashed workspaces keep them; imports in progress are protected). Orphans from interrupted imports are also reclaimed at startup.
- **Importing a package held the database lock for the whole operation** (freezing every other action) and was not atomic. Extraction/verification now happens with no lock; the workspace is created in one short transaction. Failed or cancelled imports leave no workspace and no extracted files.
- Adding items to a workspace is now one transaction: a failure no longer leaves a partial addition.
- The inspector built one `<option>` per workspace item for dependency targets and note attachments, making selection slow in large workspaces. Replaced by a filterable picker (renders ≤ 100 matches).
- Invalid HTML (`<div>` inside `<p>`) in empty/error states.
- CI no longer uses Windows runners; a stray tracked `crates/parcel-core/Cargo.lock` was removed.

### Added
- **Open `.parcel` files from Explorer** (double-click / *Open with*), including forwarding to an already-running window (`tauri-plugin-single-instance`, official Tauri plugin — the only new dependency). Only existing `.parcel` files are accepted from command lines.
- **Drop a `.parcel` onto the window** on any page outside a workspace to open it; dropping other files there explains how to add them.
- **Show in folder** for collected files (inspector, Health tab), exports, extractions and the open package. Paths come from the database or from locations the app itself produced; arbitrary paths are refused.
- **Large-collection performance:** the list view and package-explorer table render only visible rows; the grid view loads 400 items at a time; the explorer's folder outline shows only relevant folders. Sorting, search, selection (Ctrl+A, Shift+click), and keyboard navigation work across all items.
- Toasts can carry an action button.

### Measured (Linux, release build, 25,000 files; scratch benchmark, not shipped)
| | 1.0.0 | 1.1.0 |
|---|---|---|
| Import: time the database lock is held | ≈ 22.7 s (whole import) | ≈ 0.6 s (commit only) |
| Import: total | 22.7 s | 15.3 s |
| Add 25,000 files to a workspace | 2.9 s | 1.1 s |

Import time is dominated by writing and fsync-ing each extracted file; this was kept for data safety.

### Verification (Linux x86-64 container, Rust 1.91, Node 22)
| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | 68 passed, 0 failed (13 new regression tests in `parcel-workspace/tests/v110.rs`) |
| `npm run format:check`, `lint`, `typecheck` | pass |
| `npm test` | 98 passed, 0 failed (26 new) |
| `npm run build` | pass |
| Regression tests confirmed to fail when the fix is removed | yes, for atomic add, reference-aware cleanup, and failed-import cleanup |
| **Windows: double-click / Open With, second-instance forwarding, OS drag-and-drop, Show in folder, real WebView2 scrolling of windowed lists, installer** | **NOT TESTED by the author — must be validated on a Windows build** |

### Known limitations
- Virtualisation tests emulate layout in jsdom; real-browser scrolling behaviour needs checking on the Windows build.
- Removing single items from an imported workspace does not delete their stored files until the whole workspace is purged.
- Only the first of several packages passed at launch is opened.

## 1.0.0 — released

First release: the v1 foundation plus parts of the planned v2 (CLI, dependency diagnostics, revisions, batch operations, advanced search).

### Added
- `.parcel` format 1.0 (ZIP-compatible, JSON manifest, SHA-256, no absolute paths) with specification, JSON Schema and examples.
- `parcel-core` engine: atomic streaming export, hostile-input-safe reader, verification, extraction with conflict policies, ZIP import, duplicate / missing / changed source detection, dependency diagnostics, package comparison, configurable limits.
- `parcel-workspace`: SQLite persistence with migrations and recovery, workspaces (create, duplicate, trash), folders, notes, links, references, dependencies, tags, search, templates, export profiles, revisions, activity log, preferences.
- Desktop UI (Tauri 2 + React): Home, Workspaces, Open Package, Templates, Activity, Settings; three-panel editor; package explorer; light/dark themes; keyboard shortcuts.
- `parcel` CLI: inspect, verify, extract, create, validate, compare, import-zip.
- NSIS installer configuration (per-user, `.parcel` association).

### Verification results (Linux x86-64 container, Rust 1.91, Node 22)

| Check | Result |
|---|---|
| `cargo test -p parcel-core -p parcel-workspace -p parcel-cli` | 53 passed, 0 failed |
| `npm test` (Vitest, mock backend) | 72 passed, 0 failed |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy -D warnings` (core, workspace, CLI, all targets) | pass |
| `npm run format:check`, `lint`, `typecheck` | pass |
| `npm run build` (Vite production bundle) | pass |
| `cargo check -p parcel-app` (Tauri shell, Linux) | pass, 0 warnings |
| `cargo clippy -p parcel-app -D warnings` | pass |
| `tauri build` / NSIS installer | Not run by the author of this snapshot. **Later built and manually tested on Windows by the maintainer** (maintainer-reported) |
| Running the desktop app | Not run by the author. **Launched and manually tested on Windows by the maintainer** (maintainer-reported) |
| End-to-end tests through the real Tauri IPC | **NOT WRITTEN** |

### Known limitations
- Windows behaviour is untested here (junctions, alternate data streams, installer, file association, PDF preview via WebView2, OS drag-and-drop events).
- Frontend tests use an in-memory mock of the `Api` contract; they cannot detect drift from the Rust commands.
- Not implemented from the v2 plan: large-package performance work (virtualised lists), accessibility audit with assistive technology, ZIP interoperability test matrix, polished installer UX, package signing/encryption.
- Source checks are size+mtime by default; use *Deep check* to re-hash.
- The installer is unsigned.

### Fixed during verification
- CLI panicked on a closed stdout pipe (`parcel inspect x | head`); output now ignores broken pipes (regression test added).
- Settings number fields snapped to `1` while editing; values are now validated on save.
