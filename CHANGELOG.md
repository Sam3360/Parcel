# Changelog

## 1.0.0 — unreleased (verification snapshot)

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
| `tauri build` / NSIS installer | **NOT RUN** — must be built and tested on Windows |
| Running the desktop app (Linux or Windows) | **NOT RUN** — no display/Windows in the build environment |
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
