# Security

## Reporting

Report vulnerabilities privately to the maintainers (use your repository host's private security advisory feature). Include a sample package if relevant. Expect an acknowledgement, a fix and a coordinated disclosure.

## Threat model

Parcel opens files that may have been crafted by an attacker (`.parcel` and `.zip`). It must not let such a file read, write, overwrite or execute anything outside what the user explicitly chose, nor exhaust memory or disk unexpectedly.

Out of scope for 1.x: authenticity of packages (no signatures), confidentiality (no encryption), a compromised operating system, and malicious content *inside* extracted files once the user opens them with other programs.

## Controls (all covered by automated tests unless noted)

| Threat | Control |
|---|---|
| Path traversal / absolute paths / drive letters / backslashes / reserved Windows names in archive or manifest | Every member name and manifest path is validated (`pathsafe`), then joined with a containment check. Rejected at open — before any extraction. |
| Case-insensitive collisions (`A.txt` vs `a.txt`) | Rejected at scan time and in manifest validation. |
| ZIP bombs | Per-member compression-ratio limit, per-entry and total size limits, entry-count limit, manifest size limit; reads are always bounded by *declared size + 1*. All limits configurable (Settings → Security limits). |
| Lying central directory / manifest sizes | Streaming verification counts bytes actually read and compares SHA-256; mismatches abort extraction and remove the partial file. |
| Symbolic links in archives | Rejected for packages; skipped with a warning for ZIP import. |
| Symlinked destination directories | Extraction walks the destination one component at a time and refuses symlinks and non-directories. (Unit-tested on Unix; Windows junction behaviour is not separately tested.) |
| Silent overwrite | Extraction uses an explicit conflict policy (`fail` pre-flights and writes nothing; `skip`; `rename`); files are placed by atomic no-clobber rename. Export writes to a temp file and only replaces an existing package when the OS save dialog confirmed it. |
| Corrupt / partial output | Atomic export (temp file → verify → rename); cancellation and errors remove temp files. |
| Malicious links | Only `http`, `https`, `ftp`, `mailto` are accepted anywhere (entries, references, Markdown). Links open in the default browser only after a confirmation dialog. Link entries are extracted as `.url.txt`, never as launchable shortcuts. |
| Active content in previews | Previews are inert: text as text, Markdown rendered to React elements (no HTML pass-through, lint-enforced), images via `<img>` (SVG is never inlined), PDF via a `blob:` object. Package content is never executed or launched. |
| Malformed metadata | Strict schema-level validation (ids, sha256 hex, metadata key/value sizes, referential integrity, folder/file nesting). |
| Tauri attack surface | Capabilities grant only core events, window title, and file dialogs; no shell, fs, http or opener permissions are exposed to the webview. All file access goes through typed commands that require absolute paths. CSP: `default-src 'self'`, no remote scripts, `connect-src` limited to IPC. |
| Source files | Opened read-only; export checks size/timestamp (and hash when needed) first and refuses to proceed if a source is missing or changed unless the user explicitly accepts. A source modified *during* export is reported afterwards. |

## Added in 1.1.0

| Threat | Control |
|---|---|
| Crafted command line / second launch makes Parcel touch arbitrary paths | Only existing regular files ending in `.parcel` are accepted from arguments; they are merely *opened as packages* through the normal hostile-input checks. |
| Revealing arbitrary paths via the UI bridge | `reveal_source` takes an entry id and uses the path stored in the database; `reveal_output` only accepts locations the app created or opened this session. Revealing never executes anything (files are selected in the file manager; folders are opened). |
| Half-imported workspaces and orphaned extracted files | Import is stage-then-commit: workspace creation is one transaction and failed/cancelled imports delete their extracted folder. |
| Deleting files still in use | Cleanup only removes UUID-named folders that no workspace or entry references (substring match on the UUID, so path-format differences keep data), skips symlinks and in-flight imports. Covered by tests including duplicate workspaces. |
| Partial collection after a failed add | Adding to a workspace is a single transaction. |

## Added in 1.2.0

| Threat | Control |
|---|---|
| Deleting a user's own files | Parcel only ever deletes files inside its managed storage (`<app data>/imported/<workspace uuid>/…`): a file is skipped unless its normalised path is below that root and its first component is a workspace UUID. Collected files (`origin = source`) are never candidates, even if a database row claimed otherwise (tested). |
| Deleting a file another workspace still uses | A file is deleted only if no entry in any workspace — including duplicates and trashed workspaces — and no pending undo snapshot refers to it. |
| Path spelling differences (Windows `\\?\` paths, case, slashes) causing a miss | Every comparison uses one normaliser; a mismatch can only keep files. The Windows forms are unit-tested as strings on Linux; real Windows behaviour is not verified by automated tests. |
| Symlinks | Never followed, listed or deleted by cleanup or the storage scan. |
| Stale UI data driving deletion | *Remove unused files* ignores the report shown and recomputes at deletion time; imports in progress are protected. |
| Undo restoring something unsafe | Undo only re-inserts rows that were in the database moments earlier; no file content is created or read. |

## Known gaps

* No package signing or encryption.
* The fast source check trusts size + modification time; use *Deep check* to re-hash.
* Reading a file that is being modified concurrently can produce a package containing the newer bytes; the user is warned after export.
* Windows-specific filesystem edge cases (junctions, 8.3 names, alternate data streams) are not specifically tested. 1.1.0's Windows-dependent behaviour (file association launch, single-instance forwarding, revealing files in Explorer) has been tested on Linux only until validated on a Windows build.
* Undo history is in memory only: it is lost on restart and when leaving a workspace.
