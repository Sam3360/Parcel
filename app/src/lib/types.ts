// Mirrors the Rust models (snake_case over IPC).
export type EntryKind = 'file' | 'folder' | 'note' | 'link';

export interface Workspace {
  id: string;
  name: string;
  description: string;
  notes: string;
  metadata: Record<string, string>;
  package_id: string;
  revision: number;
  export_profile_id: string | null;
  template_id: string | null;
  last_export_path: string | null;
  created_at: string;
  modified_at: string;
  opened_at: string | null;
  deleted_at: string | null;
}
export interface WorkspaceSummary {
  id: string;
  name: string;
  description: string;
  entry_count: number;
  total_size: number;
  revision: number;
  modified_at: string;
  opened_at: string | null;
  deleted_at: string | null;
}
export interface Entry {
  id: string;
  workspace_id: string;
  parent_id: string | null;
  kind: EntryKind;
  name: string;
  description: string;
  url: string | null;
  content: string | null;
  attached_to: string | null;
  sort_order: number;
  origin: 'source' | 'imported';
  source_path: string | null;
  size: number;
  sha256: string | null;
  mime: string | null;
  source_mtime_ms: number | null;
  tags: string[];
  metadata: Record<string, string>;
  created_at: string;
  modified_at: string;
}
export interface EntryUpdate {
  description?: string;
  tags?: string[];
  metadata?: Record<string, string>;
  content?: string;
  url?: string;
  attached_to?: string | null;
}
export interface WorkspaceUpdate {
  name?: string;
  description?: string;
  notes?: string;
  metadata?: Record<string, string>;
  export_profile_id?: string | null;
}
export interface RefRow {
  id: string;
  workspace_id: string;
  title: string;
  url: string;
  description: string;
  category: string;
  entry_id: string | null;
  sort_order: number;
}
export interface RefInput {
  title: string;
  url: string;
  description?: string;
  category?: string;
  entry_id?: string | null;
}
export interface WorkspaceStats {
  files: number;
  folders: number;
  notes: number;
  links: number;
  references: number;
  total_size: number;
  tagged_entries: number;
  largest: [string, number][];
  by_extension: [string, number][];
}
export interface SearchQuery {
  text?: string;
  kind?: EntryKind;
  tag?: string;
  ext?: string;
  parent_id?: string;
  sort?: string;
  descending?: boolean;
  limit?: number;
}
export interface ActivityRow {
  id: number;
  ts: string;
  workspace_id: string | null;
  kind: string;
  message: string;
  detail: unknown;
}
export interface RecentPackage {
  path: string;
  name: string;
  opened_at: string;
  size: number;
}
export interface RevisionRow {
  revision: number;
  package_id: string;
  path: string;
  exported_at: string;
  entry_count: number;
  package_size: number;
}

export interface AddReport {
  added: string[];
  already_collected: string[];
  renamed: [string, string][];
  content_duplicates: [string, string][];
  warnings: string[];
}
export type SourceState =
  | { state: 'unchanged' }
  | { state: 'changed'; reason: string }
  | { state: 'missing' }
  | { state: 'unreadable'; reason: string };
export type SourceCheck = { entry_id: string; path: string; source_path: string } & SourceState;
export interface DuplicateGroup {
  sha256: string;
  size: number;
  ids: string[];
  labels: string[];
  wasted_bytes: number;
}
export interface Diagnostic {
  severity: 'error' | 'warning' | 'info';
  code: string;
  message: string;
  entry_id: string | null;
}
export interface ReplacementCheck {
  size: number;
  sha256: string;
  identical: boolean;
}
export interface ExportOutcome {
  path: string;
  package_size: number;
  content_size: number;
  entry_count: number;
  revision: number;
  excluded: number;
  warnings: string[];
}

export interface TemplateSpec {
  folders: string[];
  notes: { path: string; content: string }[];
  metadata_fields: { key: string; label: string; default: string }[];
  default_profile?: string | null;
}
export interface Template {
  id: string;
  name: string;
  description: string;
  builtin: boolean;
  spec: TemplateSpec;
}
export interface ProfileSpec {
  compress: boolean;
  verify_after_write: boolean;
  include_workspace_notes: boolean;
  include_references: boolean;
  exclude_patterns: string[];
  filename_pattern: string;
}
export interface ExportProfile {
  id: string;
  name: string;
  description: string;
  builtin: boolean;
  spec: ProfileSpec;
}

// Package (manifest) types
export interface ManifestEntry {
  id: string;
  kind: EntryKind;
  path: string;
  storage_path?: string;
  size: number;
  sha256?: string;
  mime?: string;
  url?: string;
  description: string;
  tags: string[];
  metadata: Record<string, string>;
  modified_at?: string;
}
export interface Manifest {
  format: string;
  format_version: string;
  package_id: string;
  name: string;
  description: string;
  created_at: string;
  modified_at: string;
  revision: number;
  generator: { name: string; version: string };
  entries: ManifestEntry[];
  references: {
    id: string;
    title: string;
    url: string;
    description: string;
    category: string;
    entry_id?: string;
  }[];
  dependencies: {
    id: string;
    kind: 'entry' | 'external';
    from_entry: string;
    target: string;
    description: string;
    required: boolean;
  }[];
  metadata: Record<string, string>;
}
export interface PackageInfo {
  path: string;
  file_size: number;
  manifest: Manifest;
  notes: string;
  diagnostics: Diagnostic[];
}
export type CheckResult =
  | { status: 'ok' }
  | { status: 'missing' }
  | { status: 'hash_mismatch' }
  | { status: 'size_mismatch'; expected: number; actual: number }
  | { status: 'unreadable'; reason: string };
export interface IntegrityReport {
  ok: boolean;
  checked: number;
  failures: ({ id: string; path: string } & CheckResult)[];
  metadata_failures: string[];
  unlisted: string[];
  warnings: string[];
}
export type ConflictPolicy = 'fail' | 'skip' | 'rename';
export interface ExtractReport {
  written: string[];
  skipped: string[];
  bytes_written: number;
}
export interface Preview {
  kind: 'text' | 'markdown' | 'json' | 'csv' | 'image' | 'pdf' | 'none';
  mime: string | null;
  text: string | null;
  data_base64: string | null;
  truncated: boolean;
  reason: string | null;
}
export interface EntryDiff {
  path: string;
  kind: string;
  detail: string[];
}
export interface Comparison {
  added: EntryDiff[];
  removed: EntryDiff[];
  changed: EntryDiff[];
  moved: EntryDiff[];
  metadata_changed: EntryDiff[];
  unchanged: string[];
  package_differences: string[];
}
export interface ZipImportResult {
  path: string;
  entries: number;
  size: number;
  warnings: string[];
}

export interface Progress {
  job_id: string;
  phase: string;
  current_item: string;
  items_done: number;
  items_total: number;
  bytes_done: number;
  bytes_total: number;
}
export interface IpcError {
  code: string;
  message: string;
  details?: { missing?: SourceProblem[]; changed?: SourceProblem[] } | null;
}
export interface SourceProblem {
  entry_id: string;
  path: string;
  source_path: string;
  detail: string;
}

export interface DepRow {
  id: string;
  workspace_id: string;
  from_entry: string;
  kind: 'entry' | 'external';
  target: string;
  description: string;
  required: boolean;
}

export interface RemovalReceipt {
  id: number;
  workspace_id: string;
  /** Items removed, including everything inside removed folders. */
  removed: number;
  /** Stored copies of imported files that are deleted once the removal becomes final. */
  stored_files: number;
}
export interface RemovalSummary {
  id: number;
  label: string;
  removed: number;
}
export interface UndoOutcome {
  restored: number;
  renamed: number;
  relocated: number;
}
export interface StorageOutcome {
  files_removed: number;
  bytes_freed: number;
}
export interface StorageReport {
  files: number;
  bytes: number;
  unused_files: number;
  unused_bytes: number;
  unused_examples: string[];
  orphan_folders: number;
}
