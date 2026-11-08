import type * as T from './types';

/** Everything the UI needs from the outside world. The real implementation talks to Tauri;
 *  tests inject an in-memory one. The UI never imports Tauri directly. */
export interface Api {
  // workspaces
  listWorkspaces(trashed?: boolean): Promise<T.WorkspaceSummary[]>;
  recentWorkspaces(limit?: number): Promise<T.WorkspaceSummary[]>;
  createWorkspace(name: string, description: string, templateId?: string | null): Promise<T.Workspace>;
  getWorkspace(id: string): Promise<T.Workspace>;
  updateWorkspace(id: string, update: T.WorkspaceUpdate): Promise<T.Workspace>;
  duplicateWorkspace(id: string, newName: string): Promise<T.Workspace>;
  trashWorkspace(id: string): Promise<void>;
  restoreWorkspace(id: string): Promise<void>;
  purgeWorkspace(id: string): Promise<void>;
  // entries
  listEntries(workspaceId: string): Promise<T.Entry[]>;
  createFolder(workspaceId: string, parentId: string | null, name: string): Promise<T.Entry>;
  createNote(
    workspaceId: string,
    parentId: string | null,
    name: string,
    content: string,
    attachedTo?: string | null,
  ): Promise<T.Entry>;
  createLink(
    workspaceId: string,
    parentId: string | null,
    name: string,
    url: string,
    description: string,
  ): Promise<T.Entry>;
  renameEntry(id: string, name: string): Promise<T.Entry>;
  updateEntry(id: string, update: T.EntryUpdate): Promise<T.Entry>;
  moveEntries(ids: string[], newParent: string | null): Promise<void>;
  reorderEntries(workspaceId: string, parentId: string | null, orderedIds: string[]): Promise<void>;
  deleteEntries(ids: string[]): Promise<number>;
  batchTag(ids: string[], add: string[], remove: string[]): Promise<void>;
  searchEntries(workspaceId: string, query: T.SearchQuery): Promise<T.Entry[]>;
  workspaceStats(workspaceId: string): Promise<T.WorkspaceStats>;
  // references
  listReferences(workspaceId: string): Promise<T.RefRow[]>;
  addReference(workspaceId: string, input: T.RefInput): Promise<T.RefRow>;
  updateReference(id: string, input: T.RefInput): Promise<T.RefRow>;
  deleteReference(id: string): Promise<void>;
  listDependencies(workspaceId: string): Promise<T.DepRow[]>;
  addDependency(
    workspaceId: string,
    fromEntry: string,
    kind: 'entry' | 'external',
    target: string,
    description: string,
    required: boolean,
  ): Promise<T.DepRow>;
  deleteDependency(id: string): Promise<void>;
  openExternal(url: string): Promise<void>;
  // collection & intelligence
  addPaths(
    workspaceId: string,
    parentId: string | null,
    paths: string[],
    jobId: string,
  ): Promise<T.AddReport>;
  checkSources(workspaceId: string, deep: boolean, jobId: string): Promise<T.SourceCheck[]>;
  relinkSource(entryId: string, newPath: string, acceptDifferent: boolean): Promise<T.ReplacementCheck>;
  acceptSourceChanges(entryIds: string[]): Promise<number>;
  findDuplicates(workspaceId: string): Promise<T.DuplicateGroup[]>;
  workspaceDiagnostics(workspaceId: string): Promise<T.Diagnostic[]>;
  previewEntry(entryId: string): Promise<T.Preview>;
  listRevisions(workspaceId: string): Promise<T.RevisionRow[]>;
  // export / import / packages
  exportWorkspace(
    workspaceId: string,
    dest: string,
    overwrite: boolean,
    acceptChanged: boolean,
    jobId: string,
  ): Promise<T.ExportOutcome>;
  importPackage(path: string, jobId: string): Promise<T.Workspace>;
  convertZip(
    zipPath: string,
    dest: string,
    name: string,
    overwrite: boolean,
    jobId: string,
  ): Promise<T.ZipImportResult>;
  openPackage(path: string): Promise<T.PackageInfo>;
  verifyPackage(path: string, jobId: string): Promise<T.IntegrityReport>;
  extractPackage(
    path: string,
    dest: string,
    ids: string[] | null,
    conflict: T.ConflictPolicy,
    jobId: string,
  ): Promise<T.ExtractReport>;
  previewPackageEntry(path: string, entryId: string): Promise<T.Preview>;
  comparePackages(oldPath: string, newPath: string): Promise<T.Comparison>;
  // templates / profiles
  listTemplates(): Promise<T.Template[]>;
  saveTemplate(
    id: string | null,
    name: string,
    description: string,
    spec: T.TemplateSpec,
  ): Promise<T.Template>;
  templateFromWorkspace(
    workspaceId: string,
    name: string,
    description: string,
    includeNotes: boolean,
  ): Promise<T.Template>;
  deleteTemplate(id: string): Promise<void>;
  listProfiles(): Promise<T.ExportProfile[]>;
  saveProfile(
    id: string | null,
    name: string,
    description: string,
    spec: T.ProfileSpec,
  ): Promise<T.ExportProfile>;
  deleteProfile(id: string): Promise<void>;
  // activity / recents / settings
  listActivity(workspaceId: string | null, limit: number): Promise<T.ActivityRow[]>;
  clearActivity(): Promise<void>;
  listRecentPackages(limit: number): Promise<T.RecentPackage[]>;
  removeRecentPackage(path: string): Promise<void>;
  getPrefs(): Promise<Record<string, string>>;
  setPref(key: string, value: string): Promise<void>;
  cancelJob(jobId: string): Promise<boolean>;
  appInfo(): Promise<{ version: string; data_dir: string; schema_version: number; format_version: string }>;
  // native shell (dialogs, drag-drop, progress events)
  pickFiles(): Promise<string[]>;
  pickFolder(): Promise<string | null>;
  pickPackage(): Promise<string | null>;
  pickZip(): Promise<string | null>;
  pickSaveParcel(defaultName: string): Promise<string | null>;
  pickDirectory(title: string): Promise<string | null>;
  pickAnyFile(): Promise<string | null>;
  onFileDrop(cb: (paths: string[]) => void): () => void;
  onProgress(cb: (p: T.Progress) => void): () => void;
}

let current: Api | null = null;
export function setApi(a: Api) {
  current = a;
}
export function getApi(): Api {
  if (!current) throw new Error('API not initialised');
  return current;
}

export function describeError(e: unknown): string {
  if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message);
  return String(e);
}
export function errorCode(e: unknown): string | undefined {
  return e && typeof e === 'object' && 'code' in e ? String((e as { code: unknown }).code) : undefined;
}
