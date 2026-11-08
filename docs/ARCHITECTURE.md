# Architecture

```
┌──────────────────────────── app/ (React + TypeScript) ───────────────────────────┐
│ pages / features / components  ──►  Api interface (lib/api.ts)                    │
│                                       ▲ real: lib/tauriApi.ts   ▲ tests: mockApi  │
└───────────────────────────────────────┼───────────────────────────────────────────┘
                     typed IPC (invoke + progress events)
┌───────────────────────────────────────▼───────────────────────────────────────────┐
│ app/src-tauri  — thin command layer: validate args, run long jobs on blocking      │
│ threads, throttle progress events, map errors → {code, message, details}           │
└───────────────┬───────────────────────────────────────────────┬───────────────────┘
                │                                               │
┌───────────────▼─────────────────────┐         ┌───────────────▼──────────────────┐
│ crates/parcel-workspace             │  uses   │ crates/parcel-core               │
│ SQLite store, migrations, templates │────────►│ manifest · pathsafe · limits     │
│ collect, source checks, export/     │         │ writer · reader · zipimport      │
│ import orchestration, search        │         │ intel · compare · hash · mime    │
└─────────────────────────────────────┘         └───────────────▲──────────────────┘
                                                                │
                                              crates/parcel-cli (same engine)
```

## Principles

* **Engine independence.** `parcel-core` knows nothing about databases or UIs; a package opens with no database present. `parcel-workspace` knows nothing about Tauri. Both are tested directly.
* **Linked sources, not copies.** A workspace records where each file lives plus its size, SHA-256 and mtime. Bytes are read once, at export. This keeps workspaces cheap and makes *missing/changed* detection meaningful. Files extracted from an imported package are kept in an app-managed folder (`origin = imported`) so imported workspaces are self-contained.
* **Two representations, kept apart.** The database holds the editable workspace (tree, notes, tags…). The `.parcel` file holds a frozen, self-describing snapshot. Logical paths are derived from the tree at export time and never include source paths.
* **Atomicity.** Every database mutation is a transaction; exports and extractions go through temp files and no-clobber renames.
* **Cancellation & progress.** Long operations take a `CancelToken` and a progress callback. Tauri jobs are registered by id; the UI shows a dialog after 200 ms with a Cancel button.
* **Errors are values.** Rust errors carry stable codes (`unsafe_path`, `limit_exceeded`, `source_problems`, …) that the UI maps to specific messages.

## Database (SQLite, WAL, foreign keys on)

`workspaces`, `entries` (tree via `parent_id`, kind, name, source metadata, tags/metadata JSON, note content), `refs`, `dependencies`, `activity`, `preferences`, `templates`, `export_profiles`, `recent_packages`, `revisions`. Migrations are an append-only array applied under `PRAGMA user_version`; an existing database is copied to `parcel.bak-vN` before an upgrade, a damaged database is moved aside (never deleted), and a database from a newer version is refused.

## Workspace deletion

Deleting moves a workspace to the trash (reversible). Permanent deletion only removes Parcel's records; it never touches source files or exported packages.

## Extension points (v3, not implemented)

The `Api` boundary, the `parcel.`-prefixed reserved metadata namespace and the migration hook in `manifest::migrate` are where plugins, metadata providers and format integrations would attach. No plugin system exists in 1.0.
