# Parcel

**Everything that belongs together, packaged together.**

Parcel is an offline Windows desktop app (Tauri 2 + React + Rust) for collecting files, folders, notes and links from anywhere, organising them in a workspace, and packaging everything into one portable, verifiable `.parcel` file. A `.parcel` is a documented, ZIP-compatible format that does not need Parcel to open.

- No account, no cloud, no telemetry, no analytics, no background network activity.
- Your original files are only ever **read**. Parcel never modifies, moves or deletes them.
- Imported packages are treated as hostile input (path traversal, ZIP bombs, symlinks, malformed manifests are all rejected).

> **Status:** v1.0.0 and v1.1.0 are released. **v1.2.0 is in preparation** (undo for removals, cleanup of imported files). Its changes are covered by automated tests on Linux only; Windows builds are tested manually by the maintainer and the results are not recorded in this repository. See [Verification status](#verification-status), [CHANGELOG.md](CHANGELOG.md) and [Building on Windows](#building-on-windows).

## What works

| Area | Details |
|---|---|
| Workspaces | Create (blank or from a template), rename, duplicate, trash/restore/permanently delete, recent list, everything auto-saved to SQLite. |
| Collecting | Files, whole folders, multi-select, drag-and-drop from Explorer, notes, links, descriptions, tags, custom metadata. Files stay where they are; Parcel records path, size, SHA-256, timestamp. |
| Organising | Folder tree, move (drag-and-drop or dialog), manual order, rename, multi-select, batch tag, remove-from-workspace with **undo** (Ctrl+Z; never deletes originals), list/grid views, sort, filter by type/tag, search across names, descriptions, tags, metadata and note text (`kind:` `tag:` `ext:` operators). |
| Notes & references | Markdown notes with live preview and autosave, workspace-level notes, categorised references, link entries. URLs are stored as text only and opened in your browser only after you confirm. |
| Source intelligence | Detects **missing** and **changed** sources, lets you relink (validated by checksum) or explicitly accept a change; finds **duplicate content**; blocks export until problems are resolved — nothing is replaced silently. |
| Packages | Export (streaming, atomic, verified after writing, cancellable, never overwrites without the OS save-dialog confirmation), open (from the app, by double-clicking a `.parcel`, via *Open with*, or by dropping it on the window), browse, search, preview (images, text, Markdown, JSON, CSV; PDF via the WebView viewer), verify, extract all/selected with conflict policy, import as workspace (extracted outside the database lock, created atomically; cancelled or failed imports leave nothing behind), import ZIP, compare two packages. |
| Templates & profiles | Five built-in templates, save your own (or a workspace's structure), export profiles (compression, verification, exclusions, file-name pattern). |
| Revisions | Each export of a workspace is a numbered revision of the same package lineage. |
| Dependencies | Declare entry→entry and external dependencies; cycles and dangling targets are diagnosed. |
| CLI | `parcel inspect` `verify` `extract` `create` `validate` `compare` `import-zip`, using the same engine as the app. |
| Storage | Files imported from a package are stored by Parcel; when you remove them and leave the workspace, Parcel deletes its stored copy (only if nothing else uses it). *Settings → Storage* shows how much is stored and removes leftovers on request. Files you collect from your computer are never stored or deleted by Parcel. |
| Large collections | Windowed list view and package table (only visible rows are rendered), grid view loads in chunks, filterable item picker; *Show in folder* for source files, exports and extractions. |
| UI | Light/dark/system themes, compact density, reduced motion, keyboard shortcuts (press `?`), context menus, focus-trapped dialogs, progress with cancellation. |

## Repository layout

```text
crates/parcel-core        package engine: manifest, archive writer/reader, integrity, ZIP import, comparison (no Tauri)
crates/parcel-workspace   SQLite persistence, collection, export/import orchestration, templates (no Tauri)
crates/parcel-cli         `parcel` command-line tool
app/                      React + TypeScript UI; app/src-tauri = thin Tauri 2 command layer
format/                   SPECIFICATION.md and JSON Schemas
examples/                 generated example packages (+ README)
docs/                     architecture, security, user guide, development guide
```

## Quick start (development, PowerShell)

Prerequisites: Rust ≥ 1.85 (stable, MSVC toolchain), Node.js ≥ 20, the Microsoft C++ Build Tools, and the WebView2 runtime (preinstalled on Windows 11).

```powershell
git clone <your-fork-url> parcel; cd parcel

# Engine + workspace + CLI tests (no Tauri needed)
cargo test -p parcel-core -p parcel-workspace -p parcel-cli

# Frontend
cd app
npm install
npm run typecheck; npm run lint; npm run format:check; npm test

# Run the desktop app in development
npm run tauri dev
```

### Building on Windows

```powershell
cd app
npm install
npm run tauri build          # produces the NSIS installer

# Output: target\release\bundle\nsis\Parcel_1.0.0_x64-setup.exe
# (workspace-level target directory)
```

The installer is per-user (`installMode: currentUser`), is **unsigned** (no code-signing certificate is assumed — Windows SmartScreen may warn), and registers the `.parcel` file association. CI deliberately **does not** build or upload Windows installers.

### Using the CLI

```powershell
cargo build --release -p parcel-cli      # target\release\parcel.exe
.\target\release\parcel.exe inspect examples\research-collection.parcel
.\target\release\parcel.exe verify  examples\research-collection.parcel
.\target\release\parcel.exe extract examples\research-collection.parcel out --select Data
.\target\release\parcel.exe create  my.parcel .\some-folder --name "My folder"
```

## Verification status

See [CHANGELOG.md](CHANGELOG.md) for the exact test results recorded for this release and the list of known limitations.

The v1.0.0 codebase has been tested on Linux, and the Windows build has been compiled successfully. The Windows desktop application has also been launched and manually tested. The NSIS installer has been generated successfully (all reported by the maintainer).

The v1.1.0 changes are covered by automated tests on Linux (see CHANGELOG). Behaviour that depends on Windows — file association launches, the single-instance handoff, OS drag-and-drop, revealing files in Explorer, and real WebView2 scrolling of virtualised lists — can only be confirmed by testing the Windows build.

## Documentation

- [Package format specification](format/SPECIFICATION.md) · [JSON Schema](format/schemas/manifest.schema.json) · [examples](examples/README.md)
- [User guide](docs/USER_GUIDE.md) · [Architecture](docs/ARCHITECTURE.md) · [Security](docs/SECURITY.md) · [Development guide](docs/DEVELOPMENT.md) · [Contributing](CONTRIBUTING.md)

## License

MIT — see [LICENSE](LICENSE).
