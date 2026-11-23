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

## Importing a package (two phases)

1. **Stage** (`StagedImport::prepare`): open the package (all hostile-input checks), read notes, extract and verify files into `…/imported/<new workspace uuid>/`. No database access, so the Tauri layer holds no lock. Any failure or cancellation removes the folder before returning.
2. **Commit** (`Store::commit_import`): one SQLite transaction creates the workspace, folders, entries, attachments, references and dependencies. If it fails, SQLite rolls back and the staged folder is discarded. Only files are extracted; notes live in the database.

The workspace id of an import in flight is registered as *protected* so no cleanup can touch its folder.

## Managed-file cleanup

`Store::cleanup_managed_dirs` runs after a permanent delete and at startup. A folder under `imported/` is removed only if it is a real (non-symlink) directory named as a lower-case UUID, is not protected, has no workspace row (trashed workspaces count), and no entry in any workspace has a source path containing that UUID. The UUID substring test is deliberately tolerant of path-prefix differences (Windows `\\?\` paths), so uncertainty keeps files rather than deleting them. Duplicated workspaces share extracted files, so the folder survives until the last workspace using it is purged.

## Opening packages from outside

`parcel_workspace::launch` parses command-line arguments (only existing regular `.parcel` files are accepted). At startup, and from the `tauri-plugin-single-instance` callback when a second copy is launched, the paths go into a queue in `AppState`; the UI drains it with `take_launch_packages` (also triggered by a `parcel://open-package` event). The window is un-minimised and focused. Show-in-folder uses database paths (`reveal_source`) or a small allow-list of paths the app itself produced or opened (`reveal_output`).

## Large lists

`lib/virtual.tsx` windows fixed-height table rows inside the `.content-scroll` container (row height is measured, spacer rows preserve scroll height). Sorting, filtering and selection happen upstream on the full item array; keyboard navigation scrolls by index. Below 80 rows, or when no layout is available, everything renders.

## Undoable removal and stored-file cleanup (`removal.rs`)

`Store::remove_entries` deletes the rows in one transaction and keeps an in-memory snapshot (entries, references, dependencies, attachments) per workspace, up to 20 deep. **Nothing is persisted**, so there is no schema change and a crash simply loses the ability to undo. `undo_remove` re-inserts the snapshot atomically (parents first, deferred foreign keys): items whose folder disappeared go to the workspace root, name clashes get a numeric suffix, links from references and notes are re-attached.

Stored copies of imported files (`origin = "imported"`) are deleted only when a removal becomes **final** — history overflow, leaving the workspace (`finalize_removals`), or app exit — and only if the file lies under `<managed root>/<workspace uuid>/` and no entry in any workspace and no pending undo refers to it. Paths are compared through `norm_path` (strips `\\?\`, unifies slashes, lower-cases), so differences in spelling can only make paths compare equal and keep the file. Relinking an imported item to the user's own file frees the stored copy by the same rules. `scan_managed_storage` / `remove_unused_storage` back *Settings → Storage* and recompute everything at deletion time.

## Workspace deletion

Deleting moves a workspace to the trash (reversible). Permanent deletion only removes Parcel's records; it never touches source files or exported packages.

## Extension points (v3, not implemented)

The `Api` boundary, the `parcel.`-prefixed reserved metadata namespace and the migration hook in `manifest::migrate` are where plugins, metadata providers and format integrations would attach. No plugin system exists in 1.0.
