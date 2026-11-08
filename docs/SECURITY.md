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

## Known gaps

* No package signing or encryption.
* The fast source check trusts size + modification time; use *Deep check* to re-hash.
* Reading a file that is being modified concurrently can produce a package containing the newer bytes; the user is warned after export.
* The Windows build has not been exercised by the maintainers of this snapshot; Windows-specific filesystem edge cases (junctions, 8.3 names, alternate data streams) are not specifically tested.
