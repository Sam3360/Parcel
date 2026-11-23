import type { Api } from '../lib/api';
import type * as T from '../lib/types';

let n = 0;
const id = (p = 'id') => `${p}-${++n}`;
const now = () => new Date().toISOString();
export class MockError extends Error {
  constructor(
    public code: string,
    message: string,
    public details?: unknown,
  ) {
    super(message);
  }
}

export interface MockState {
  workspaces: T.Workspace[];
  entries: T.Entry[];
  refs: T.RefRow[];
  deps: T.DepRow[];
  prefs: Record<string, string>;
  templates: T.Template[];
  profiles: T.ExportProfile[];
  activity: T.ActivityRow[];
  recentPackages: T.RecentPackage[];
  sources: Record<string, T.SourceState>;
  packages: Record<string, T.PackageInfo>;
  picks: {
    files: string[];
    folder: string | null;
    pkg: string | null;
    save: string | null;
    dir: string | null;
    any: string | null;
  };
  exportProblems: { missing: T.SourceProblem[]; changed: T.SourceProblem[] } | null;
  verify: Record<string, T.IntegrityReport>;
  exported: { dest: string; accept: boolean }[];
  launch: string[];
  /** Undo history per workspace (snapshots of removed entries), newest last. */
  removals: Record<string, { id: number; label: string; entries: T.Entry[] }[]>;
  /** Workspaces whose pending removals were finalized (stored copies freed). */
  finalized: string[];
  storage: T.StorageReport;
  storageRemoved: number;
  revealed: { kind: 'source' | 'output'; value: string }[];
  dropHandlers: Set<(p: string[]) => void>;
  openHandlers: Set<() => void>;
  extracted: { path: string; dest: string; ids: string[] | null; policy: string }[];
}

export function baseTemplates(): T.Template[] {
  return [
    {
      id: 'tpl-research',
      name: 'Research collection',
      description: 'Sources and findings.',
      builtin: true,
      spec: {
        folders: ['Sources', 'Data'],
        notes: [{ path: 'Notes/Question.md', content: '# Question' }],
        metadata_fields: [{ key: 'topic', label: 'Topic', default: '' }],
      },
    },
    {
      id: 'tpl-documents',
      name: 'General document collection',
      description: 'Documents.',
      builtin: true,
      spec: { folders: ['Documents'], notes: [], metadata_fields: [] },
    },
  ];
}
export function baseProfiles(): T.ExportProfile[] {
  const spec: T.ProfileSpec = {
    compress: true,
    verify_after_write: true,
    include_workspace_notes: true,
    include_references: true,
    exclude_patterns: [],
    filename_pattern: '{name}',
  };
  return [
    { id: 'prof-standard', name: 'Standard', description: 'Compressed and verified.', builtin: true, spec },
  ];
}

export function makeEntry(p: Partial<T.Entry> & Pick<T.Entry, 'workspace_id' | 'name' | 'kind'>): T.Entry {
  return {
    id: id('e'),
    parent_id: null,
    description: '',
    url: null,
    content: null,
    attached_to: null,
    sort_order: ++n,
    origin: 'source',
    source_path: null,
    size: 0,
    sha256: null,
    mime: null,
    source_mtime_ms: null,
    tags: [],
    metadata: {},
    created_at: now(),
    modified_at: now(),
    ...p,
  };
}

export function createMockApi(
  overrides: Partial<Api> = {},
  seed: Partial<MockState> = {},
): { api: Api; state: MockState } {
  const s: MockState = {
    workspaces: [],
    entries: [],
    refs: [],
    deps: [],
    prefs: {},
    templates: baseTemplates(),
    profiles: baseProfiles(),
    activity: [],
    recentPackages: [],
    sources: {},
    packages: {},
    verify: {},
    picks: { files: [], folder: null, pkg: null, save: '/out/Test.parcel', dir: null, any: null },
    exportProblems: null,
    exported: [],
    extracted: [],
    launch: [],
    removals: {},
    finalized: [],
    storage: { files: 0, bytes: 0, unused_files: 0, unused_bytes: 0, unused_examples: [], orphan_folders: 0 },
    storageRemoved: 0,
    revealed: [],
    dropHandlers: new Set(),
    openHandlers: new Set(),
    ...seed,
  };
  const ws = (wid: string) => {
    const w = s.workspaces.find((x) => x.id === wid);
    if (!w) throw new MockError('not_found', `workspace ${wid} not found`);
    return w;
  };
  const ent = (eid: string) => {
    const e = s.entries.find((x) => x.id === eid);
    if (!e) throw new MockError('not_found', 'entry not found');
    return e;
  };
  const log = (kind: string, message: string) =>
    s.activity.unshift({
      id: s.activity.length + 1,
      ts: now(),
      workspace_id: null,
      kind,
      message,
      detail: null,
    });
  const touch = (wid: string) => {
    ws(wid).modified_at = now();
  };
  const sibling = (wid: string, parent: string | null, name: string, except?: string) =>
    s.entries.some(
      (e) =>
        e.workspace_id === wid &&
        e.parent_id === parent &&
        e.id !== except &&
        e.name.toLowerCase() === name.toLowerCase(),
    );
  const okName = (name: string) => {
    if (!name.trim() || /[\\/]/.test(name)) throw new MockError('invalid', 'invalid name');
  };
  const unique = (wid: string, parent: string | null, name: string) => {
    let c = name,
      i = 0;
    while (sibling(wid, parent, c)) c = name.replace(/(\.[^.]*)?$/, ` (${++i})$1`);
    return c;
  };
  const summary = (w: T.Workspace): T.WorkspaceSummary => {
    const es = s.entries.filter((e) => e.workspace_id === w.id);
    return {
      id: w.id,
      name: w.name,
      description: w.description,
      entry_count: es.length,
      total_size: es.reduce((a, e) => a + e.size, 0),
      revision: w.revision,
      modified_at: w.modified_at,
      opened_at: w.opened_at,
      deleted_at: w.deleted_at,
    };
  };
  const descendants = (eid: string): string[] =>
    s.entries.filter((e) => e.parent_id === eid).flatMap((c) => [c.id, ...descendants(c.id)]);
  const pathOf = (e: T.Entry): string => (e.parent_id ? `${pathOf(ent(e.parent_id))}/${e.name}` : e.name);

  const api: Api = {
    listWorkspaces: async (trashed = false) =>
      s.workspaces.filter((w) => !!w.deleted_at === trashed).map(summary),
    recentWorkspaces: async () => s.workspaces.filter((w) => w.opened_at && !w.deleted_at).map(summary),
    createWorkspace: async (name, description, templateId) => {
      if (!name.trim()) throw new MockError('invalid', 'workspace name must be 1-200 characters');
      const w: T.Workspace = {
        id: id('ws'),
        name: name.trim(),
        description,
        notes: '',
        metadata: {},
        package_id: id('pkg'),
        revision: 0,
        export_profile_id: null,
        template_id: templateId ?? null,
        last_export_path: null,
        created_at: now(),
        modified_at: now(),
        opened_at: null,
        deleted_at: null,
      };
      s.workspaces.push(w);
      const t = s.templates.find((x) => x.id === templateId);
      t?.spec.folders.forEach((f) =>
        s.entries.push(makeEntry({ workspace_id: w.id, name: f, kind: 'folder' })),
      );
      log('workspace.created', `Created workspace “${w.name}”`);
      return w;
    },
    getWorkspace: async (wid) => {
      const w = ws(wid);
      w.opened_at = now();
      return { ...w };
    },
    updateWorkspace: async (wid, u) => {
      const w = ws(wid);
      if (u.name !== undefined && !u.name.trim()) throw new MockError('invalid', 'name required');
      Object.assign(w, u, { modified_at: now() });
      return { ...w };
    },
    duplicateWorkspace: async (wid, name) => {
      const src = ws(wid);
      const w = await api.createWorkspace(name, src.description, null);
      s.entries
        .filter((e) => e.workspace_id === wid)
        .forEach((e) => s.entries.push({ ...e, id: id('e'), workspace_id: w.id }));
      return w;
    },
    trashWorkspace: async (wid) => {
      ws(wid).deleted_at = now();
    },
    restoreWorkspace: async (wid) => {
      ws(wid).deleted_at = null;
    },
    purgeWorkspace: async (wid) => {
      if (!ws(wid).deleted_at) throw new MockError('invalid', 'only trashed workspaces can be deleted');
      s.workspaces = s.workspaces.filter((w) => w.id !== wid);
      s.entries = s.entries.filter((e) => e.workspace_id !== wid);
    },

    listEntries: async (wid) => s.entries.filter((e) => e.workspace_id === wid).map((e) => ({ ...e })),
    createFolder: async (wid, parent, name) => {
      okName(name);
      if (sibling(wid, parent, name))
        throw new MockError('name_conflict', `a sibling named “${name}” already exists here`);
      const e = makeEntry({ workspace_id: wid, parent_id: parent, name, kind: 'folder' });
      s.entries.push(e);
      touch(wid);
      return e;
    },
    createNote: async (wid, parent, name, content, att) => {
      okName(name);
      const nm = unique(wid, parent, name.endsWith('.md') ? name : `${name}.md`);
      const e = makeEntry({
        workspace_id: wid,
        parent_id: parent,
        name: nm,
        kind: 'note',
        content,
        size: content.length,
        attached_to: att ?? null,
        mime: 'text/markdown',
      });
      s.entries.push(e);
      touch(wid);
      return e;
    },
    createLink: async (wid, parent, name, url, description) => {
      if (!/^(https?|ftp):\/\/|^mailto:/i.test(url))
        throw new MockError('invalid', 'only http, https, ftp and mailto links are supported');
      const e = makeEntry({
        workspace_id: wid,
        parent_id: parent,
        name: unique(wid, parent, name),
        kind: 'link',
        url,
        description,
      });
      s.entries.push(e);
      return e;
    },
    renameEntry: async (eid, name) => {
      const e = ent(eid);
      okName(name);
      if (sibling(e.workspace_id, e.parent_id, name, eid))
        throw new MockError('name_conflict', `a sibling named “${name}” already exists here`);
      e.name = name;
      return { ...e };
    },
    updateEntry: async (eid, u) => {
      const e = ent(eid);
      if (u.content !== undefined) {
        e.content = u.content;
        e.size = u.content.length;
      }
      Object.assign(e, {
        description: u.description ?? e.description,
        tags: u.tags ?? e.tags,
        metadata: u.metadata ?? e.metadata,
        url: u.url ?? e.url,
        attached_to: u.attached_to === undefined ? e.attached_to : u.attached_to,
        modified_at: now(),
      });
      return { ...e };
    },
    moveEntries: async (ids, parent) => {
      for (const i of ids) {
        const e = ent(i);
        if (parent && (parent === i || descendants(i).includes(parent)))
          throw new MockError('invalid', 'cannot move a folder into itself');
        if (sibling(e.workspace_id, parent, e.name, i))
          throw new MockError('name_conflict', `a sibling named “${e.name}” already exists here`);
      }
      ids.forEach((i) => {
        ent(i).parent_id = parent;
      });
    },
    reorderEntries: async (_w, _p, ordered) => {
      ordered.forEach((i, k) => {
        ent(i).sort_order = k;
      });
    },
    deleteEntries: async (ids) => {
      const all = new Set(ids.flatMap((i) => [i, ...descendants(i)]));
      const removed = s.entries.filter((e) => all.has(e.id));
      if (removed.length === 0) throw new MockError('not_found', 'entry not found');
      const ws = removed[0].workspace_id;
      const stack = (s.removals[ws] ??= []);
      const rec = {
        id: ++n,
        label: ids.length === 1 ? removed.find((e) => e.id === ids[0])!.name : `${ids.length} items`,
        entries: removed.map((e) => ({ ...e })),
      };
      stack.push(rec);
      s.entries = s.entries.filter((e) => !all.has(e.id));
      return {
        id: rec.id,
        workspace_id: ws,
        removed: removed.length,
        stored_files: removed.filter((e) => e.kind === 'file' && e.origin === 'imported').length,
      };
    },
    undoRemove: async (wid) => {
      const stack = s.removals[wid] ?? [];
      const rec = stack[stack.length - 1];
      if (!rec) throw new MockError('invalid', 'there is nothing to undo');
      if (ws(wid).deleted_at)
        throw new MockError('invalid', 'restore the workspace from the trash before undoing');
      const ids = new Set(rec.entries.map((e) => e.id));
      const out = { restored: 0, renamed: 0, relocated: 0 };
      for (const e of rec.entries) {
        const row = { ...e };
        if (!e.parent_id || !ids.has(e.parent_id)) {
          if (e.parent_id && !s.entries.some((x) => x.id === e.parent_id && x.kind === 'folder')) {
            row.parent_id = null;
            out.relocated++;
          }
          const uname = unique(wid, row.parent_id, row.name);
          if (uname !== row.name) {
            row.name = uname;
            out.renamed++;
          }
        }
        s.entries.push(row);
        out.restored++;
      }
      stack.pop();
      return out;
    },
    listRemovals: async (wid) =>
      [...(s.removals[wid] ?? [])]
        .reverse()
        .map((r) => ({ id: r.id, label: r.label, removed: r.entries.length })),
    finalizeRemovals: async (wid) => {
      s.finalized.push(wid);
      delete s.removals[wid];
      return { files_removed: 0, bytes_freed: 0 };
    },
    storageReport: async () => ({ ...s.storage }),
    removeUnusedStorage: async () => {
      s.storageRemoved++;
      const freed = s.storage.unused_bytes;
      s.storage = { ...s.storage, unused_files: 0, unused_bytes: 0, unused_examples: [], orphan_folders: 0 };
      return { files_removed: 1, bytes_freed: freed };
    },
    batchTag: async (ids, add, remove) => {
      ids.forEach((i) => {
        const e = ent(i);
        e.tags = [...e.tags.filter((t) => !remove.includes(t)), ...add.filter((t) => !e.tags.includes(t))];
      });
    },
    searchEntries: async (wid, q) => {
      const terms = (q.text ?? '').toLowerCase().split(/\s+/).filter(Boolean);
      return s.entries
        .filter(
          (e) =>
            e.workspace_id === wid &&
            (!q.kind || e.kind === q.kind) &&
            (!q.tag || e.tags.includes(q.tag)) &&
            terms.every((t) =>
              `${pathOf(e)} ${e.description} ${e.tags.join(' ')} ${e.content ?? ''}`
                .toLowerCase()
                .includes(t),
            ),
        )
        .map((e) => ({ ...e }));
    },
    workspaceStats: async (wid) => {
      const es = s.entries.filter((e) => e.workspace_id === wid);
      return {
        files: es.filter((e) => e.kind === 'file').length,
        folders: es.filter((e) => e.kind === 'folder').length,
        notes: es.filter((e) => e.kind === 'note').length,
        links: es.filter((e) => e.kind === 'link').length,
        references: s.refs.filter((r) => r.workspace_id === wid).length,
        total_size: es.reduce((a, e) => a + e.size, 0),
        tagged_entries: 0,
        largest: [],
        by_extension: [],
      };
    },

    listReferences: async (wid) => s.refs.filter((r) => r.workspace_id === wid),
    addReference: async (wid, i) => {
      if (!/^(https?|ftp):\/\//i.test(i.url))
        throw new MockError('invalid', 'only http, https, ftp and mailto URLs are supported');
      const r: T.RefRow = {
        id: id('r'),
        workspace_id: wid,
        title: i.title,
        url: i.url,
        description: i.description ?? '',
        category: i.category ?? '',
        entry_id: i.entry_id ?? null,
        sort_order: n,
      };
      s.refs.push(r);
      return r;
    },
    updateReference: async (rid, i) => {
      const r = s.refs.find((x) => x.id === rid)!;
      Object.assign(r, i);
      return r;
    },
    deleteReference: async (rid) => {
      s.refs = s.refs.filter((r) => r.id !== rid);
    },
    listDependencies: async (wid) => s.deps.filter((d) => d.workspace_id === wid),
    addDependency: async (wid, from, kind, target, description, required) => {
      const d: T.DepRow = {
        id: id('d'),
        workspace_id: wid,
        from_entry: from,
        kind,
        target,
        description,
        required,
      };
      s.deps.push(d);
      return d;
    },
    deleteDependency: async (did) => {
      s.deps = s.deps.filter((d) => d.id !== did);
    },
    openExternal: async () => {},
    takeLaunchPackages: async () => s.launch.splice(0, s.launch.length),
    revealSource: async (eid) => {
      s.revealed.push({ kind: 'source', value: eid });
    },
    revealOutput: async (p) => {
      s.revealed.push({ kind: 'output', value: p });
    },

    addPaths: async (wid, parent, paths) => {
      const rep: T.AddReport = {
        added: [],
        already_collected: [],
        renamed: [],
        content_duplicates: [],
        warnings: [],
      };
      for (const p of paths) {
        const name = p.split(/[\\/]/).pop()!;
        if (s.entries.some((e) => e.workspace_id === wid && e.source_path === p)) {
          rep.already_collected.push(name);
          continue;
        }
        const nm = unique(wid, parent, name);
        if (nm !== name) rep.renamed.push([name, nm]);
        s.entries.push(
          makeEntry({
            workspace_id: wid,
            parent_id: parent,
            name: nm,
            kind: 'file',
            source_path: p,
            size: 100 + nm.length,
            sha256: `hash-${name}`,
            mime: nm.endsWith('.png') ? 'image/png' : 'text/plain',
          }),
        );
        rep.added.push(nm);
      }
      return rep;
    },
    checkSources: async (wid) =>
      s.entries
        .filter((e) => e.workspace_id === wid && e.kind === 'file')
        .map((e) => ({
          entry_id: e.id,
          path: pathOf(e),
          source_path: e.source_path ?? '',
          ...(s.sources[e.id] ?? { state: 'unchanged' as const }),
        })),
    relinkSource: async (eid, p, accept) => {
      const e = ent(eid);
      if (!accept && p.includes('different')) throw new MockError('invalid', 'content differs');
      e.source_path = p;
      delete s.sources[eid];
      return { size: 1, sha256: 'x', identical: !p.includes('different') };
    },
    acceptSourceChanges: async (ids) => {
      ids.forEach((i) => delete s.sources[i]);
      return ids.length;
    },
    findDuplicates: async (wid) => {
      const m = new Map<string, T.Entry[]>();
      s.entries
        .filter((e) => e.workspace_id === wid && e.sha256)
        .forEach((e) => m.set(e.sha256!, [...(m.get(e.sha256!) ?? []), e]));
      return [...m.entries()]
        .filter(([, v]) => v.length > 1)
        .map(([h, v]) => ({
          sha256: h,
          size: v[0].size,
          ids: v.map((e) => e.id),
          labels: v.map(pathOf),
          wasted_bytes: v[0].size * (v.length - 1),
        }));
    },
    workspaceDiagnostics: async () => [],
    previewEntry: async (eid) => {
      const e = ent(eid);
      return e.kind === 'note'
        ? { kind: 'markdown', mime: null, text: e.content, data_base64: null, truncated: false, reason: null }
        : {
            kind: 'text',
            mime: null,
            text: `contents of ${e.name}`,
            data_base64: null,
            truncated: false,
            reason: null,
          };
    },
    listRevisions: async (wid) =>
      ws(wid).revision > 0
        ? [
            {
              revision: ws(wid).revision,
              package_id: 'p',
              path: ws(wid).last_export_path ?? '',
              exported_at: now(),
              entry_count: 1,
              package_size: 1,
            },
          ]
        : [],

    exportWorkspace: async (wid, dest, overwrite, accept) => {
      if (s.exportProblems && !(accept && s.exportProblems.missing.length === 0))
        throw new MockError('source_problems', 'sources missing or changed', s.exportProblems);
      const w = ws(wid);
      w.revision += 1;
      w.last_export_path = dest;
      s.exported.push({ dest, accept });
      void overwrite;
      return {
        path: dest,
        package_size: 2048,
        content_size: 1024,
        entry_count: s.entries.filter((e) => e.workspace_id === wid).length,
        revision: w.revision,
        excluded: 0,
        warnings: [],
      };
    },
    importPackage: async (path) =>
      api.createWorkspace(s.packages[path]?.manifest.name ?? 'Imported', '', null),
    convertZip: async (_z, dest) => ({ path: dest, entries: 3, size: 100, warnings: [] }),
    openPackage: async (path) => {
      const p = s.packages[path];
      if (!p) throw new MockError('not_a_package', 'not a valid Parcel package: manifest.json is missing');
      return p;
    },
    verifyPackage: async (path) =>
      s.verify[path] ?? {
        ok: true,
        checked: s.packages[path]?.manifest.entries.filter((e) => e.sha256).length ?? 0,
        failures: [],
        metadata_failures: [],
        unlisted: [],
        warnings: [],
      },
    extractPackage: async (path, dest, ids, policy) => {
      s.extracted.push({ path, dest, ids, policy });
      return { written: ['a'], skipped: [], bytes_written: 10 };
    },
    previewPackageEntry: async (_p, eid) => ({
      kind: 'text',
      mime: null,
      text: `package entry ${eid}`,
      data_base64: null,
      truncated: false,
      reason: null,
    }),
    comparePackages: async () => ({
      added: [{ path: 'new.txt', kind: 'file', detail: [] }],
      removed: [],
      changed: [{ path: 'a.txt', kind: 'file', detail: ['size 1 → 2 bytes'] }],
      moved: [],
      metadata_changed: [],
      unchanged: ['b.txt'],
      package_differences: [],
    }),

    listTemplates: async () => s.templates,
    saveTemplate: async (tid, name, description, spec) => {
      const t = { id: tid ?? id('tpl'), name, description, builtin: false, spec };
      s.templates.push(t);
      return t;
    },
    templateFromWorkspace: async (wid, name) => {
      const t = {
        id: id('tpl'),
        name,
        description: '',
        builtin: false,
        spec: {
          folders: s.entries.filter((e) => e.workspace_id === wid && e.kind === 'folder').map((e) => e.name),
          notes: [],
          metadata_fields: [],
        },
      };
      s.templates.push(t);
      return t;
    },
    deleteTemplate: async (tid) => {
      s.templates = s.templates.filter((t) => t.id !== tid);
    },
    listProfiles: async () => s.profiles,
    saveProfile: async (pid, name, description, spec) => {
      const p = { id: pid ?? id('prof'), name, description, builtin: false, spec };
      s.profiles.push(p);
      return p;
    },
    deleteProfile: async (pid) => {
      s.profiles = s.profiles.filter((p) => p.id !== pid);
    },

    listActivity: async () => s.activity,
    clearActivity: async () => {
      s.activity = [];
    },
    listRecentPackages: async () => s.recentPackages,
    removeRecentPackage: async (p) => {
      s.recentPackages = s.recentPackages.filter((r) => r.path !== p);
    },
    getPrefs: async () => ({ ...s.prefs }),
    setPref: async (k, v) => {
      s.prefs[k] = v;
    },
    cancelJob: async () => true,
    appInfo: async () => ({ version: '1.0.0', data_dir: '/data', schema_version: 1, format_version: '1.0' }),

    pickFiles: async () => s.picks.files,
    pickFolder: async () => s.picks.folder,
    pickPackage: async () => s.picks.pkg,
    pickZip: async () => null,
    pickSaveParcel: async () => s.picks.save,
    pickDirectory: async () => s.picks.dir,
    pickAnyFile: async () => s.picks.any,
    onFileDrop: (cb) => {
      s.dropHandlers.add(cb);
      return () => {
        s.dropHandlers.delete(cb);
      };
    },
    onProgress: () => () => {},
    onOpenPackage: (cb) => {
      s.openHandlers.add(cb);
      return () => {
        s.openHandlers.delete(cb);
      };
    },
    ...overrides,
  };
  return { api, state: s };
}

export function samplePackage(path = '/pk/Demo.parcel'): T.PackageInfo {
  const e = (
    p: Partial<T.ManifestEntry> & { id: string; path: string; kind: T.EntryKind },
  ): T.ManifestEntry => ({ size: 0, description: '', tags: [], metadata: {}, ...p });
  return {
    path,
    file_size: 4096,
    notes: '# Package notes\nHello **world**',
    diagnostics: [],
    manifest: {
      format: 'parcel',
      format_version: '1.0',
      package_id: 'pid',
      name: 'Demo package',
      description: 'A demo',
      created_at: now(),
      modified_at: now(),
      revision: 2,
      generator: { name: 'parcel-core', version: '1.0.0' },
      entries: [
        e({ id: 'f1', path: 'Docs', kind: 'folder' }),
        e({
          id: 'a',
          path: 'Docs/report.txt',
          kind: 'file',
          size: 120,
          sha256: 'a'.repeat(64),
          mime: 'text/plain',
          description: 'The report',
        }),
        e({ id: 'b', path: 'image.png', kind: 'file', size: 900, sha256: 'b'.repeat(64), mime: 'image/png' }),
        e({ id: 'l', path: 'Site', kind: 'link', url: 'https://example.com' }),
      ],
      references: [
        { id: 'r1', title: 'Rust docs', url: 'https://doc.rust-lang.org', description: '', category: 'Docs' },
      ],
      dependencies: [],
      metadata: {},
    },
  };
}
