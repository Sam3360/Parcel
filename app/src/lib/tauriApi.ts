import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open, save } from '@tauri-apps/plugin-dialog';
import type { Api } from './api';
import type { Progress } from './types';

const j = <T>(cmd: string, args?: Record<string, unknown>) => invoke<T>(cmd, args);

export const tauriApi: Api = {
  listWorkspaces: (trashed = false) => j('list_workspaces', { trashed }),
  recentWorkspaces: (limit = 6) => j('recent_workspaces', { limit }),
  createWorkspace: (name, description, templateId = null) =>
    j('create_workspace', { name, description, templateId }),
  getWorkspace: (id) => j('get_workspace', { id }),
  updateWorkspace: (id, update) => j('update_workspace', { id, update }),
  duplicateWorkspace: (id, newName) => j('duplicate_workspace', { id, newName }),
  trashWorkspace: (id) => j('trash_workspace', { id }),
  restoreWorkspace: (id) => j('restore_workspace', { id }),
  purgeWorkspace: (id) => j('purge_workspace', { id }),
  listEntries: (workspaceId) => j('list_entries', { workspaceId }),
  createFolder: (workspaceId, parentId, name) => j('create_folder', { workspaceId, parentId, name }),
  createNote: (workspaceId, parentId, name, content, attachedTo = null) =>
    j('create_note', { workspaceId, parentId, name, content, attachedTo }),
  createLink: (workspaceId, parentId, name, url, description) =>
    j('create_link', { workspaceId, parentId, name, url, description }),
  renameEntry: (id, name) => j('rename_entry', { id, name }),
  updateEntry: (id, update) => j('update_entry', { id, update }),
  moveEntries: (ids, newParent) => j('move_entries', { ids, newParent }),
  reorderEntries: (workspaceId, parentId, orderedIds) =>
    j('reorder_entries', { workspaceId, parentId, orderedIds }),
  deleteEntries: (ids) => j('delete_entries', { ids }),
  batchTag: (ids, add, remove) => j('batch_tag', { ids, add, remove }),
  searchEntries: (workspaceId, query) => j('search_entries', { workspaceId, query }),
  workspaceStats: (workspaceId) => j('workspace_stats', { workspaceId }),
  listReferences: (workspaceId) => j('list_references', { workspaceId }),
  addReference: (workspaceId, input) => j('add_reference', { workspaceId, input }),
  updateReference: (id, input) => j('update_reference', { id, input }),
  deleteReference: (id) => j('delete_reference', { id }),
  listDependencies: (workspaceId) => j('list_dependencies', { workspaceId }),
  addDependency: (workspaceId, fromEntry, kind, target, description, required) =>
    j('add_dependency', { workspaceId, fromEntry, kind, target, description, required }),
  deleteDependency: (id) => j('delete_dependency', { id }),
  openExternal: (url) => j('open_external', { url }),
  addPaths: (workspaceId, parentId, paths, jobId) => j('add_paths', { workspaceId, parentId, paths, jobId }),
  checkSources: (workspaceId, deep, jobId) => j('check_sources', { workspaceId, deep, jobId }),
  relinkSource: (entryId, newPath, acceptDifferent) =>
    j('relink_source', { entryId, newPath, acceptDifferent }),
  acceptSourceChanges: (entryIds) => j('accept_source_changes', { entryIds }),
  findDuplicates: (workspaceId) => j('find_duplicates', { workspaceId }),
  workspaceDiagnostics: (workspaceId) => j('workspace_diagnostics', { workspaceId }),
  previewEntry: (entryId) => j('preview_entry', { entryId }),
  listRevisions: (workspaceId) => j('list_revisions', { workspaceId }),
  exportWorkspace: (workspaceId, dest, overwrite, acceptChanged, jobId) =>
    j('export_workspace', { workspaceId, dest, overwrite, acceptChanged, jobId }),
  importPackage: (path, jobId) => j('import_package', { path, jobId }),
  convertZip: (zipPath, dest, name, overwrite, jobId) =>
    j('convert_zip', { zipPath, dest, name, overwrite, jobId }),
  openPackage: (path) => j('open_package', { path }),
  verifyPackage: (path, jobId) => j('verify_package', { path, jobId }),
  extractPackage: (path, dest, ids, conflict, jobId) =>
    j('extract_package', { path, dest, ids, conflict, jobId }),
  previewPackageEntry: (path, entryId) => j('preview_package_entry', { path, entryId }),
  comparePackages: (oldPath, newPath) => j('compare_packages', { old: oldPath, new: newPath }),
  listTemplates: () => j('list_templates'),
  saveTemplate: (id, name, description, spec) => j('save_template', { id, name, description, spec }),
  templateFromWorkspace: (workspaceId, name, description, includeNotes) =>
    j('template_from_workspace', { workspaceId, name, description, includeNotes }),
  deleteTemplate: (id) => j('delete_template', { id }),
  listProfiles: () => j('list_profiles'),
  saveProfile: (id, name, description, spec) => j('save_profile', { id, name, description, spec }),
  deleteProfile: (id) => j('delete_profile', { id }),
  listActivity: (workspaceId, limit) => j('list_activity', { workspaceId, limit }),
  clearActivity: () => j('clear_activity'),
  listRecentPackages: (limit) => j('list_recent_packages', { limit }),
  removeRecentPackage: (path) => j('remove_recent_package', { path }),
  getPrefs: () => j('get_prefs'),
  setPref: (key, value) => j('set_pref', { key, value }),
  cancelJob: (jobId) => j('cancel_job', { jobId }),
  appInfo: () => j('app_info'),

  pickFiles: async () => {
    const r = await open({ multiple: true, directory: false, title: 'Add files' });
    return Array.isArray(r) ? r : r ? [r] : [];
  },
  pickFolder: async () => {
    const r = await open({ multiple: false, directory: true, title: 'Add a folder' });
    return typeof r === 'string' ? r : null;
  },
  pickPackage: async () => {
    const r = await open({
      multiple: false,
      directory: false,
      title: 'Open package',
      filters: [{ name: 'Parcel package', extensions: ['parcel'] }],
    });
    return typeof r === 'string' ? r : null;
  },
  pickZip: async () => {
    const r = await open({
      multiple: false,
      directory: false,
      title: 'Import ZIP archive',
      filters: [{ name: 'ZIP archive', extensions: ['zip'] }],
    });
    return typeof r === 'string' ? r : null;
  },
  pickSaveParcel: async (defaultPath) => {
    const r = await save({
      title: 'Save package',
      defaultPath,
      filters: [{ name: 'Parcel package', extensions: ['parcel'] }],
    });
    return r ?? null;
  },
  pickDirectory: async (title) => {
    const r = await open({ multiple: false, directory: true, title });
    return typeof r === 'string' ? r : null;
  },
  pickAnyFile: async () => {
    const r = await open({ multiple: false, directory: false, title: 'Choose file' });
    return typeof r === 'string' ? r : null;
  },
  onFileDrop: (cb) => {
    let un: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        if (ev.payload.type === 'drop') cb(ev.payload.paths);
      })
      .then((u) => {
        if (cancelled) u();
        else un = u;
      });
    return () => {
      cancelled = true;
      un?.();
    };
  },
  onProgress: (cb) => {
    let un: (() => void) | undefined;
    let cancelled = false;
    listen<Progress>('parcel://progress', (e) => cb(e.payload)).then((u) => {
      if (cancelled) u();
      else un = u;
    });
    return () => {
      cancelled = true;
      un?.();
    };
  },
};
