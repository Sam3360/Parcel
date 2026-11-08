# The Parcel package format — version 1.0

Status: stable for 1.x. This document is normative; the key words MUST, SHOULD and MAY are used as in RFC 2119.

A **Parcel package** (`.parcel`) is an ordinary ZIP archive with a fixed layout and a JSON manifest. It is designed so that:

* any ZIP tool can open it and see the files (the format is **independent of the Parcel application**);
* a reader can decide, before extracting a single byte, whether the package is safe and intact;
* nothing about the creator's machine (absolute paths, user names) leaks into it by default;
* the format can evolve without breaking older readers.

## 1. Container

* The file MUST be a valid ZIP archive (PKWARE APPNOTE; ZIP64 permitted for members ≥ 4 GiB).
* Compression methods: *stored* (0) and *deflate* (8). Writers SHOULD store already-compressed media (PNG, JPEG, MP4, ZIP, OOXML) uncompressed. Readers MAY reject other methods.
* Encrypted members are **not** supported and MUST be rejected.
* Member names are UTF-8, use `/` as separator, and MUST satisfy the path rules in §4.
* Symbolic links MUST NOT appear. Readers MUST reject archives containing them.
* Writers SHOULD place `manifest.json` last (it is produced after all content has been hashed); readers MUST locate it by name via the central directory and MUST NOT rely on member order.

## 2. Layout

```
Example.parcel
├── manifest.json                 required — the authoritative index
├── content/                      stored bytes of file and note entries
│   └── <logical path>
├── metadata/
│   ├── notes.md                  workspace-level Markdown notes (optional)
│   └── references.json           mirror of manifest.references (optional)
└── previews/
    └── thumbnail.png             optional thumbnail
```

Members not described by the manifest are **unlisted**. Readers MUST ignore unlisted members for all purposes except reporting them, and MUST NOT extract them.

The stored path of an entry is always `content/` + its logical path, so a plain ZIP extraction of `content/` reproduces the logical folder structure.

## 3. Manifest (`manifest.json`)

UTF-8 JSON, validated by [`schemas/manifest.schema.json`](schemas/manifest.schema.json). Unknown properties MUST be ignored (forward compatibility) and SHOULD be preserved by tools that rewrite a manifest.

| Field | Type | Req. | Meaning |
|---|---|---|---|
| `format` | string | yes | Always `"parcel"`. |
| `format_version` | string `MAJOR.MINOR` | yes | See §9. |
| `package_id` | UUID | yes | Identifies the *lineage* of a package. Re-exports of the same workspace keep the same id. |
| `name` | string (1–512) | yes | Display name. |
| `description` | string | no | |
| `created_at`, `modified_at` | RFC 3339 | yes | `modified_at` is the time this revision was written. |
| `revision` | integer ≥ 1 | no (default 1) | Increases each time the lineage is exported. |
| `generator` | `{name, version}` | yes | Informational. |
| `entries` | array | no | See §3.1. |
| `references` | array | no | See §6. |
| `dependencies` | array | no | See §7. |
| `metadata_files` | array | no | Size and checksum of each `metadata/*` member. |
| `metadata` | map string→string | no | Package-level custom metadata. |

### 3.1 Entries

| Field | Meaning |
|---|---|
| `id` | Unique within the package (1–128 chars). Stable across revisions of the same lineage where the entry persists. |
| `kind` | `file`, `folder`, `note` or `link`. |
| `path` | Logical path (§4). Unique case-insensitively. |
| `storage_path` | `file`/`note` only: member name, MUST start with `content/` and be unique. MUST be absent for `folder`. |
| `size` | Exact uncompressed byte length. |
| `sha256` | Lower- or upper-case hex SHA-256 of the exact bytes. REQUIRED for `file` and `note`; MUST be absent for `folder`. |
| `mime` | Best-effort media type; informational only, never trusted for behaviour. |
| `url` | `link` only. See §6 for allowed schemes. |
| `description`, `tags`, `metadata` | Free-form. Metadata keys are 1–256 chars, values ≤ 64 KiB. Keys beginning `parcel.` are reserved (e.g. `parcel.attached_to` = id of the entry a note is attached to). |
| `modified_at` | Source modification time, if known. |

A path used by a `file`, `note` or `link` MUST NOT also be an ancestor of another entry's path (a file cannot act as a folder). Folders MAY be implicit: a reader MUST tolerate files whose parent folders have no explicit `folder` entry.

Entries of kind `note` hold Markdown (UTF-8). Entries of kind `link` hold an address only; nothing is stored in `content/`.

## 4. Paths

A logical path (and every archive member name) MUST:

1. be non-empty and at most 1024 bytes, with at most 64 components;
2. use `/` as the only separator — backslashes are forbidden;
3. be relative: no leading `/`, no `//`, no drive prefix such as `C:`;
4. contain no `.` or `..` component and no empty component (a single trailing `/` is allowed on folder members);
5. contain no control characters (U+0000–U+001F, U+007F);
6. contain none of `: * ? " < > |` (so the package extracts identically on Windows, macOS and Linux);
7. have no component ending in `.` or space, and no component whose stem is a Windows reserved device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`);
8. be unique **case-insensitively** within the archive and within the manifest.

Writers MUST sanitise user-visible names to satisfy these rules. Readers MUST reject the entire package if any member name or manifest path violates them.

## 5. Integrity

* Every `file` and `note` entry carries `size` and `sha256`. A reader verifying a package MUST stream each member, stop reading after `size + 1` bytes, and compare both values.
* ZIP-level CRC-32 values are a transport check only; they MUST NOT be relied on for integrity.
* Package-level `metadata_files` carry the same data for `metadata/*` members.
* Checksums supplied by a package are **claims**, not proofs of authenticity. SHA-256 here detects corruption and accidental modification; it does not authenticate the author. (Signing is out of scope for 1.x.)

## 6. References and links

`references[]` entries have `id`, `title`, `url` and optional `description`, `category`, `entry_id` (which MUST name an existing entry). The same URL rules apply to `link` entries.

Allowed URL schemes: `http`, `https`, `ftp`, `mailto`. All others (`javascript:`, `file:`, `data:`, `vbscript:`, custom handlers…) MUST be rejected. URLs MUST NOT contain control characters and are limited to 4096 bytes.

Readers MUST NOT fetch, prefetch or probe URLs. A URL may be handed to the user's browser only as the result of an explicit user action.

## 7. Dependencies

`dependencies[]` declare relationships a consumer might care about:

* `kind: "entry"` — `from_entry` relies on another entry in this package; `target` MUST be that entry's `id`.
* `kind: "external"` — `from_entry` relies on something outside the package; `target` is free text (for example `Python 3.12`). External dependencies are declarative only and are never resolved or fetched.

`required` (default true) marks hard dependencies. Cycles among entry dependencies are legal but SHOULD be reported as warnings.

## 8. Reader security requirements

Packages are untrusted input. A conforming reader MUST:

1. reject encrypted members, symbolic links, unsafe or colliding names (§4) and malformed manifests **before** extracting anything;
2. enforce configurable limits and report which was exceeded: number of entries, per-entry and total uncompressed size, manifest size, path length/depth, and per-member compression ratio (the reference implementation defaults to 100 000 entries, 32 GiB per entry, 64 GiB total, 64 MiB manifest and a 1000:1 ratio above 1 MiB);
3. treat declared sizes as untrusted: bound every read by the declared size + 1;
4. never execute, open, preview-with-active-content or launch package content;
5. never overwrite an existing file silently; extraction MUST resolve conflicts by an explicit policy (fail, skip, or keep both);
6. never follow pre-existing symbolic links in the destination, and write files via a temporary name followed by an atomic no-clobber rename;
7. write nothing, or clean up completely, when extraction fails or is cancelled.

A conforming writer MUST write to a temporary file in the destination directory, fsync, verify, and only then rename into place, and MUST NOT replace an existing package unless the user explicitly confirmed it. Writers MUST NOT record absolute source paths.

## 9. Versioning and migration

`format_version` is `MAJOR.MINOR`.

* **MINOR** increments add optional fields only. Readers of the same MAJOR MUST accept any MINOR and ignore unknown fields.
* **MAJOR** increments may change semantics. A reader MUST refuse a package whose MAJOR exceeds what it supports (the reference reader reports `unsupported_version`) and SHOULD offer migration for lower MAJORs. The reference implementation routes every manifest through a `migrate()` step before validation so that future MAJOR upgrades have a single place to live.

## 10. Interoperability notes

* Importing a plain ZIP: a tool may wrap any ZIP in a package by hashing each file and generating a manifest (the reference implementation provides `parcel import-zip`). `__MACOSX/` resources are skipped; backslash separators are normalised; symlinks are dropped with a warning; unsafe paths abort the import.
* Extracting with a generic unzip tool yields `content/…` plus metadata; the manifest explains what each file is.
* Link entries are extracted by Parcel as `<name>.url.txt` text files (title, URL, description). They are intentionally *not* `.url` shortcut files, which Windows would launch on double-click.

## 11. Example manifest

```json
{
  "format": "parcel",
  "format_version": "1.0",
  "package_id": "6f1c2f3e-0d4a-4c0e-9d5b-2a5f6f7f9c11",
  "name": "Example",
  "created_at": "2026-01-01T09:00:00Z",
  "modified_at": "2026-01-01T09:00:00Z",
  "revision": 1,
  "generator": { "name": "parcel-core", "version": "1.0.0" },
  "entries": [
    { "id": "f1", "kind": "folder", "path": "Docs" },
    { "id": "e1", "kind": "file", "path": "Docs/report.txt", "storage_path": "content/Docs/report.txt",
      "size": 25, "sha256": "…64 hex characters…", "mime": "text/plain", "tags": ["q3"] },
    { "id": "l1", "kind": "link", "path": "Links/Rust", "url": "https://www.rust-lang.org" }
  ],
  "references": [ { "id": "r1", "title": "Rust", "url": "https://www.rust-lang.org", "category": "docs", "entry_id": "e1" } ],
  "metadata_files": [ { "path": "metadata/notes.md", "size": 15, "sha256": "…" } ]
}
```

See [`../examples`](../examples) for complete, verifiable packages.

## 12. Validation tooling

* `parcel validate <manifest.json | package>` — checks structure and every rule above that can be checked without extracting.
* `parcel verify <package>` — full checksum verification.
* [`schemas/manifest.schema.json`](schemas/manifest.schema.json) — JSON Schema (2020-12) for manifest structure. The schema cannot express every rule (case-insensitive uniqueness, referential integrity, nesting under non-folders); the reference validator is authoritative.
