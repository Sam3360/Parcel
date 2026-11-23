# Changelog

## 1.2.0 — unreleased

Backward-compatible release. **Package format stays 1.0 and the database schema stays v1** (no migration; undo history lives in memory only). Packages and databases move freely between 1.0, 1.1 and 1.2.

### Added
- **Undo for "remove from workspace".** Ctrl+Z, the *Undo* button on the removal toast, or *Undo remove* in the toolbar. Restores the items, everything inside removed folders, and their links to references, dependencies and attached notes — exactly (same ids, order, tags, metadata, timestamps). If the original folder is gone the item returns to the top level; if its name was taken it comes back with a number. Up to 20 removals deep; the history lives while the app runs and is cleared when you leave the workspace. Works for removals from the Health tab ("remove extra copies") too.
- **Storage cleanup for imported workspaces.** Removing a file that came from an imported package now frees Parcel's stored copy once the removal is final (leaving the workspace, quitting, or more than 20 removals) — never earlier, so undo always works — and only if nothing else uses it (duplicates, other workspaces, pending undo). Previously stored copies stayed until the whole workspace was purged.
- **Settings → Storage:** shows how much imported-file storage Parcel holds and what nothing uses (e.g. after a crash), and removes it on request. Deletion is recomputed at the moment you confirm.
- The removal confirmation now says plainly when selected files are Parcel-stored copies (the imported package itself is never changed) and mentions undo.

### Changed
- Relinking an imported item to your own file now marks it as your file (it previously kept the "imported" label) and frees the stored copy if unused.
- The removal toast has an *Undo* action; toasts with actions stay up longer.

### Safety (file deletion is the riskiest thing Parcel does, so)
- Parcel only deletes files **inside its own managed storage**, in a workspace's UUID folder, that no entry in any workspace and no pending undo references. Your collected files are never candidates, even if a database row claimed otherwise.
- All path comparisons go through one normaliser (Windows `\\?\` prefix, slashes, case); a spelling difference can only cause a file to be **kept**.
- Symlinks are never followed, listed or deleted.
- Tests were mutation-checked: removing each of the reference check, root guard, undo pinning and case-folding makes specific tests fail. (The first run exposed that the root guard was not independently tested; a test was added.)

### Fixed
- Found by tests while building undo: restoring must re-link references that survived the removal.
- Removed the stray `parcel-1.1.0.patch` that had been committed to the repository root.

### Verification (Linux x86-64 container, Rust 1.91, Node 22)
| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | 91 passed, 0 failed (23 new in `parcel-workspace`) |
| `npm run format:check`, `lint`, `typecheck` | pass |
| `npm test` | 114 passed, 0 failed (16 new) |
| `npm run build` | pass |
| **Windows:** Ctrl+Z / toolbar / toast Undo, storage freed after leaving a workspace and on quit, Settings → Storage, behaviour with real `\\?\` stored paths, installer | **NOT TESTED by the author — validate on a Windows build** |

### Known limitations
- Undo history is in memory: lost on restart, on leaving the workspace, and on app exit. Stored copies of removed files are deleted at that point.
- If the app is killed (crash, power loss) before pending removals are finalized, their stored copies remain until you use *Settings → Storage → Remove unused files*.
- The Windows path forms the normaliser handles are unit-tested as strings on Linux; real Windows file behaviour is untested here.

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
