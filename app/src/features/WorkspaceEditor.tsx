import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useApp } from '../hooks/app';
import { Button, EmptyState, ErrorText, Loading } from '../components/ui';
import { Icon } from '../components/Icon';
import { ContextMenu, type MenuItem } from '../components/ContextMenu';
import { describeError } from '../lib/api';
import { isTyping } from '../lib/shortcuts';
import { formatBytes, plural, uid } from '../lib/format';
import type {
  RemovalReceipt,
  DepRow,
  Entry,
  ExportProfile,
  RefRow,
  SourceCheck,
  SourceState,
  Workspace,
  WorkspaceStats,
} from '../lib/types';
import { Tree } from './editor/Tree';
import { GridView, ListView, type ViewApi } from './editor/EntryViews';
import { Inspector } from './editor/Inspector';
import { Health } from './editor/Health';
import { References } from './editor/References';
import { ExportDialog, LinkDialog, MoveDialog, NoteEditor, exportSummary } from './editor/dialogs';

type Tab = 'items' | 'refs' | 'health';

export function WorkspaceEditor({ id, onBack }: { id: string; onBack: () => void }) {
  const { api, prefs, prompt, confirm, toast, reportError, runJob } = useApp();
  const [ws, setWs] = useState<Workspace | null>(null);
  const [entries, setEntries] = useState<Entry[]>([]);
  const [refs, setRefs] = useState<RefRow[]>([]);
  const [deps, setDeps] = useState<DepRow[]>([]);
  const [stats, setStats] = useState<WorkspaceStats | null>(null);
  const [profiles, setProfiles] = useState<ExportProfile[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [checks, setChecks] = useState<SourceCheck[] | null>(null);
  const [undoCount, setUndoCount] = useState(0);

  const [folder, setFolder] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const anchor = useRef<string | null>(null);
  const [view, setView] = useState<'list' | 'grid'>(prefs.view === 'grid' ? 'grid' : 'list');
  const [sort, setSort] = useState({ key: 'name', desc: false });
  const [search, setSearch] = useState('');
  const [kind, setKind] = useState('');
  const [tag, setTag] = useState('');
  const [results, setResults] = useState<Entry[] | null>(null);
  const [tab, setTab] = useState<Tab>('items');
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const [dialog, setDialog] = useState<'note' | 'wsnotes' | 'link' | 'editlink' | 'move' | 'export' | null>(
    null,
  );
  const [noteTarget, setNoteTarget] = useState<Entry | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const centerRef = useRef<HTMLDivElement>(null);
  const viewApi = useRef<ViewApi | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [w, e, r, d, s] = await Promise.all([
        api.getWorkspace(id),
        api.listEntries(id),
        api.listReferences(id),
        api.listDependencies(id),
        api.workspaceStats(id),
      ]);
      setWs(w);
      setEntries(e);
      setRefs(r);
      setDeps(d);
      setStats(s);
      setLoadError(null);
    } catch (e) {
      setLoadError(describeError(e));
    }
  }, [api, id]);

  const checkSources = useCallback(
    async (deep: boolean) => {
      try {
        setChecks(await api.checkSources(id, deep, uid()));
      } catch (e) {
        reportError(e, 'Could not check sources');
      }
    },
    [api, id, reportError],
  );

  useEffect(() => {
    void refresh().then(() => checkSources(false));
    api
      .listProfiles()
      .then(setProfiles)
      .catch(() => {});
  }, [refresh, checkSources, api]);

  const refreshUndo = useCallback(async () => {
    try {
      setUndoCount((await api.listRemovals(id)).length);
    } catch {
      setUndoCount(0);
    }
  }, [api, id]);

  const undo = useCallback(async () => {
    try {
      const o = await api.undoRemove(id);
      await refresh();
      await refreshUndo();
      const extra = [
        o.renamed ? `${o.renamed} renamed because the name was taken` : '',
        o.relocated ? `${o.relocated} placed at the top level because their folder is gone` : '',
      ]
        .filter(Boolean)
        .join('; ');
      toast(`Restored ${plural(o.restored, 'item')}${extra ? ` (${extra})` : ''}`, 'success');
    } catch (e) {
      reportError(e, 'Could not undo');
      await refreshUndo();
    }
  }, [api, id, refresh, refreshUndo, toast, reportError]);

  const announceRemoval = useCallback(
    (r: RemovalReceipt) => {
      void refreshUndo();
      toast(`Removed ${plural(r.removed, 'item')} from the workspace`, 'info', {
        label: 'Undo',
        onClick: () => void undo(),
      });
    },
    [refreshUndo, toast, undo],
  );

  // Leaving the workspace makes pending removals final: stored copies of removed imported files are freed.
  useEffect(() => {
    void refreshUndo();
    return () => {
      void api.finalizeRemovals(id).catch(() => {});
    };
  }, [api, id, refreshUndo]);

  const paths = useMemo(() => {
    const byId = new Map(entries.map((e) => [e.id, e]));
    const m = new Map<string, string>();
    for (const e of entries) {
      const parts = [e.name];
      let c = e.parent_id ? byId.get(e.parent_id) : undefined;
      let g = 0;
      while (c && g++ < 100) {
        parts.unshift(c.name);
        c = c.parent_id ? byId.get(c.parent_id) : undefined;
      }
      m.set(e.id, parts.join('/'));
    }
    return m;
  }, [entries]);
  const status = useMemo(
    () => new Map<string, SourceState>((checks ?? []).map((c) => [c.entry_id, c as SourceState])),
    [checks],
  );
  const problemCount = (checks ?? []).filter((c) => c.state !== 'unchanged').length;
  const byId = useMemo(() => new Map(entries.map((e) => [e.id, e])), [entries]);
  const tags = useMemo(() => {
    const m = new Map<string, number>();
    entries.forEach((e) => e.tags.forEach((t) => m.set(t, (m.get(t) ?? 0) + 1)));
    return [...m.entries()].sort();
  }, [entries]);

  // server-side search / filtering
  const filtering = !!(search.trim() || kind || tag);
  useEffect(() => {
    if (!filtering) {
      setResults(null);
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      api
        .searchEntries(id, {
          text: search,
          kind: (kind || undefined) as Entry['kind'] | undefined,
          tag: tag || undefined,
        })
        .then((r) => live && setResults(r))
        .catch((e) => live && reportError(e, 'Search failed'));
    }, 180);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [api, id, search, kind, tag, filtering, entries, reportError]);

  const items = useMemo(() => {
    const base = filtering ? (results ?? []) : entries.filter((e) => e.parent_id === folder);
    const dir = sort.desc ? -1 : 1;
    return [...base].sort((a, b) => {
      if ((a.kind === 'folder') !== (b.kind === 'folder')) return a.kind === 'folder' ? -1 : 1;
      const c =
        sort.key === 'size'
          ? a.size - b.size
          : sort.key === 'modified'
            ? a.modified_at.localeCompare(b.modified_at)
            : sort.key === 'kind'
              ? a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name)
              : sort.key === 'manual'
                ? a.sort_order - b.sort_order
                : a.name.localeCompare(b.name, undefined, { sensitivity: 'base', numeric: true });
      return c * dir;
    });
  }, [entries, results, filtering, folder, sort]);

  const sel = useMemo(
    () => [...selected].map((i) => byId.get(i)).filter(Boolean) as Entry[],
    [selected, byId],
  );
  useEffect(() => {
    setSelected((s) => new Set([...s].filter((i) => byId.has(i))));
    if (folder && !byId.has(folder)) setFolder(null);
  }, [byId, folder]);

  // ---------- adding ----------
  const addPaths = useCallback(
    async (paths: string[]) => {
      if (!paths.length) return;
      try {
        const rep = await runJob('Adding to workspace', (jobId) => api.addPaths(id, folder, paths, jobId));
        const bits = [`Added ${plural(rep.added.length, 'file')}`];
        if (rep.already_collected.length)
          bits.push(`${rep.already_collected.length} already in this workspace`);
        if (rep.renamed.length) bits.push(`${rep.renamed.length} renamed to avoid a clash`);
        toast(bits.join(' · '), 'success');
        if (rep.content_duplicates.length)
          toast(
            `${plural(rep.content_duplicates.length, 'file')} ${rep.content_duplicates.length === 1 ? 'has' : 'have'} identical content to something already collected. See the Health tab.`,
            'warn',
          );
        if (rep.warnings.length) toast(`${rep.warnings.length} item(s) skipped: ${rep.warnings[0]}`, 'warn');
        await refresh();
        await checkSources(false);
      } catch (e) {
        if ((e as { code?: string }).code === 'cancelled') toast('Cancelled');
        else reportError(e, 'Could not add');
      }
    },
    [api, id, folder, runJob, toast, refresh, checkSources, reportError],
  );
  const addPathsRef = useRef(addPaths);
  addPathsRef.current = addPaths;
  useEffect(() => api.onFileDrop((p) => void addPathsRef.current(p)), [api]);

  const pickFiles = async () => addPaths(await api.pickFiles());
  const pickFolder = async () => {
    const f = await api.pickFolder();
    if (f) await addPaths([f]);
  };
  const newFolder = async () => {
    const name = await prompt({ title: 'New folder', label: 'Folder name', confirmLabel: 'Create' });
    if (!name) return;
    try {
      const f = await api.createFolder(id, folder, name);
      await refresh();
      setSelected(new Set([f.id]));
    } catch (e) {
      reportError(e, 'Could not create folder');
    }
  };
  const newNote = async () => {
    const name = await prompt({ title: 'New note', label: 'Note title', confirmLabel: 'Create' });
    if (!name) return;
    try {
      const n = await api.createNote(id, folder, name, `# ${name}\n\n`, null);
      await refresh();
      setSelected(new Set([n.id]));
      setNoteTarget(n);
      setDialog('note');
    } catch (e) {
      reportError(e, 'Could not create note');
    }
  };

  // ---------- item actions ----------
  const rename = async (e: Entry) => {
    const n = await prompt({
      title: `Rename “${e.name}”`,
      label: 'New name',
      initial: e.name,
      confirmLabel: 'Rename',
    });
    if (!n || n === e.name) return;
    try {
      await api.renameEntry(e.id, n);
      await refresh();
    } catch (err) {
      reportError(err, 'Could not rename');
    }
  };
  const remove = async (list: Entry[]) => {
    if (!list.length) return;
    if (prefs.confirm_remove !== 'false') {
      const folders = list.filter((e) => e.kind === 'folder').length;
      const gone = new Set(list.map((e) => e.id));
      for (let grew = true; grew;) {
        grew = false;
        for (const e of entries) {
          if (e.parent_id && gone.has(e.parent_id) && !gone.has(e.id)) {
            gone.add(e.id);
            grew = true;
          }
        }
      }
      const importedCount = entries.filter(
        (e) => gone.has(e.id) && e.kind === 'file' && e.origin === 'imported',
      ).length;
      const ok = await confirm({
        title: `Remove ${plural(list.length, 'item')} from this workspace?`,
        confirmLabel: 'Remove',
        danger: true,
        message: (
          <>
            <p>
              {list.length === 1 ? `“${list[0].name}”` : `These ${list.length} items`} will be removed from
              the workspace{folders ? ', including everything inside the selected folders' : ''}.
            </p>
            <p className="muted">Your original files are not deleted or changed.</p>
            {importedCount > 0 ? (
              <p className="muted">
                {plural(importedCount, 'file')} came from an imported package, so Parcel keeps its own copy.
                Parcel deletes that copy when you leave this workspace; the package you imported from is not
                changed. Until then you can undo (Ctrl+Z).
              </p>
            ) : (
              <p className="muted">You can undo this (Ctrl+Z).</p>
            )}
          </>
        ),
      });
      if (!ok) return;
    }
    try {
      const receipt = await api.deleteEntries(list.map((e) => e.id));
      setSelected(new Set());
      await refresh();
      announceRemoval(receipt);
    } catch (e) {
      reportError(e);
    }
  };
  const move = async (ids: string[], target: string | null) => {
    try {
      await api.moveEntries(ids, target);
      await refresh();
    } catch (e) {
      reportError(e, 'Could not move');
    }
  };
  const nudge = async (e: Entry, delta: number) => {
    const sibs = entries
      .filter((x) => x.parent_id === e.parent_id)
      .sort((a, b) => a.sort_order - b.sort_order)
      .map((x) => x.id);
    const i = sibs.indexOf(e.id),
      j = i + delta;
    if (j < 0 || j >= sibs.length) return;
    [sibs[i], sibs[j]] = [sibs[j], sibs[i]];
    try {
      await api.reorderEntries(id, e.parent_id, sibs);
      await refresh();
    } catch (err) {
      reportError(err);
    }
  };
  const bulk = async (a: 'tag' | 'move' | 'remove') => {
    if (a === 'remove') return remove(sel);
    if (a === 'move') return setDialog('move');
    const t = await prompt({
      title: 'Add tag',
      label: `Tag for ${plural(sel.length, 'item')}`,
      confirmLabel: 'Add tag',
    });
    if (t) {
      try {
        await api.batchTag(
          sel.map((e) => e.id),
          [t],
          [],
        );
        await refresh();
      } catch (e) {
        reportError(e);
      }
    }
  };
  const open = (e: Entry) => {
    if (e.kind === 'folder') {
      setSearch('');
      setKind('');
      setTag('');
      setFolder(e.id);
      setSelected(new Set());
    } else if (e.kind === 'note') {
      setNoteTarget(e);
      setDialog('note');
    } else if (e.kind === 'link') setSelected(new Set([e.id]));
  };

  const click = (ev: React.MouseEvent, eid: string) => {
    if (ev.ctrlKey || ev.metaKey)
      setSelected((s) => {
        const n = new Set(s);
        if (n.has(eid)) n.delete(eid);
        else n.add(eid);
        return n;
      });
    else if (ev.shiftKey && anchor.current) {
      const a = items.findIndex((x) => x.id === anchor.current),
        b = items.findIndex((x) => x.id === eid);
      if (a >= 0 && b >= 0) {
        setSelected(new Set(items.slice(Math.min(a, b), Math.max(a, b) + 1).map((x) => x.id)));
        return;
      }
    } else setSelected(new Set([eid]));
    anchor.current = eid;
  };

  const itemMenu = (list: Entry[]): MenuItem[] => {
    const one = list.length === 1 ? list[0] : null;
    return [
      ...(one
        ? [
            {
              label: one.kind === 'folder' ? 'Open folder' : one.kind === 'note' ? 'Edit note' : 'Open',
              icon: 'folder' as const,
              onSelect: () => open(one),
              disabled: one.kind === 'file' || one.kind === 'link',
            },
            { label: 'Rename…', icon: 'edit' as const, shortcut: 'F2', onSelect: () => rename(one) },
          ]
        : []),
      { label: 'Move to…', icon: 'folder', onSelect: () => setDialog('move') },
      { label: 'Add tag…', icon: 'tag', onSelect: () => bulk('tag') },
      ...(one && sort.key === 'manual' && !filtering
        ? [
            { label: 'Move up', onSelect: () => nudge(one, -1) },
            { label: 'Move down', onSelect: () => nudge(one, 1) },
          ]
        : []),
      { separator: true },
      {
        label: 'Remove from workspace…',
        icon: 'trash',
        danger: true,
        shortcut: 'Del',
        onSelect: () => remove(list),
      },
    ];
  };
  const addItems = (): MenuItem[] => [
    { label: 'Files…', icon: 'file', shortcut: 'Ctrl+Shift+A', onSelect: pickFiles },
    { label: 'Folder from disk…', icon: 'folder', onSelect: pickFolder },
    { separator: true },
    { label: 'New folder…', icon: 'folder', shortcut: 'Ctrl+Shift+N', onSelect: newFolder },
    { label: 'New note…', icon: 'note', shortcut: 'Ctrl+N', onSelect: newNote },
    { label: 'Add link…', icon: 'link', onSelect: () => setDialog('link') },
  ];
  const showContext = (e: React.MouseEvent, eid: string | null) => {
    e.preventDefault();
    if (eid) {
      const list = selected.has(eid) ? sel : [byId.get(eid)!];
      if (!selected.has(eid)) setSelected(new Set([eid]));
      setMenu({ x: e.clientX, y: e.clientY, items: itemMenu(list) });
    } else setMenu({ x: e.clientX, y: e.clientY, items: addItems() });
  };

  // ---------- keyboard ----------
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (document.querySelector('[role=dialog]')) return;
      const k = e.key.toLowerCase();
      if (e.ctrlKey && k === 'f') {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.ctrlKey && e.shiftKey && k === 'n') {
        e.preventDefault();
        void newFolder();
      } else if (e.ctrlKey && !e.shiftKey && k === 'n') {
        e.preventDefault();
        void newNote();
      } else if (e.ctrlKey && e.shiftKey && k === 'a') {
        e.preventDefault();
        void pickFiles();
      } else if (e.ctrlKey && k === 'e') {
        e.preventDefault();
        setDialog('export');
      } else if (e.ctrlKey && !e.shiftKey && k === 'z' && !isTyping(e.target)) {
        e.preventDefault();
        void undo();
      } else if (!isTyping(e.target) && tab === 'items') {
        const idx = items.findIndex((x) => selected.has(x.id));
        if (e.ctrlKey && k === 'a') {
          e.preventDefault();
          setSelected(new Set(items.map((x) => x.id)));
        } else if (e.key === 'Delete' && sel.length) {
          e.preventDefault();
          void remove(sel);
        } else if (e.key === 'F2' && sel.length === 1) {
          e.preventDefault();
          void rename(sel[0]);
        } else if (e.key === 'Enter' && sel.length === 1) {
          e.preventDefault();
          open(sel[0]);
        } else if (e.key === 'Backspace' && !filtering && folder) {
          e.preventDefault();
          setFolder(byId.get(folder)?.parent_id ?? null);
        } else if (e.key === 'Escape') setSelected(new Set());
        else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault();
          const next =
            items[
              Math.max(0, Math.min(items.length - 1, idx < 0 ? 0 : idx + (e.key === 'ArrowDown' ? 1 : -1)))
            ];
          if (next) {
            if (e.shiftKey) setSelected((s) => new Set([...s, next.id]));
            else setSelected(new Set([next.id]));
            anchor.current = next.id;
            viewApi.current?.scrollToId(next.id);
          }
        } else if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
          if (sel.length) {
            e.preventDefault();
            const r = document.querySelector(`[data-entry-id="${sel[0].id}"]`)?.getBoundingClientRect();
            setMenu({ x: r?.left ?? 300, y: r?.bottom ?? 200, items: itemMenu(sel) });
          }
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  if (loadError)
    return (
      <div className="page">
        <EmptyState
          icon="alert"
          title="Could not open this workspace"
          action={<Button onClick={onBack}>Back to workspaces</Button>}
        >
          <ErrorText error={loadError} />
        </EmptyState>
      </div>
    );
  if (!ws) return <Loading label="Opening workspace…" />;

  const crumbs: Entry[] = [];
  for (let c = folder ? byId.get(folder) : undefined; c; c = c.parent_id ? byId.get(c.parent_id) : undefined)
    crumbs.unshift(c);
  const totalSize = items.reduce((n, e) => n + (e.kind === 'file' || e.kind === 'note' ? e.size : 0), 0);
  const viewProps = {
    items,
    selected,
    paths: filtering ? paths : null,
    status,
    sort,
    onSort: (key: string) => setSort((s) => ({ key, desc: s.key === key ? !s.desc : false })),
    onClick: click,
    onOpen: open,
    onContext: showContext,
    apiRef: viewApi,
  };

  return (
    <div className="editor">
      <div className="toolbar" role="toolbar" aria-label="Workspace tools">
        <Button
          variant="ghost"
          icon="right"
          aria-label="Back to workspaces"
          className="icon"
          onClick={onBack}
          style={{ transform: 'scaleX(-1)' }}
        />
        <button
          className="title ellipsis btn ghost"
          style={{ height: 30, fontWeight: 650, fontSize: 15 }}
          title="Rename workspace"
          onClick={async () => {
            const n = await prompt({
              title: 'Rename workspace',
              label: 'Name',
              initial: ws.name,
              confirmLabel: 'Rename',
            });
            if (n) {
              try {
                await api.updateWorkspace(id, { name: n });
                await refresh();
              } catch (e) {
                reportError(e);
              }
            }
          }}
        >
          {ws.name}
        </button>
        <Button
          icon="plus"
          onClick={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            setMenu({ x: r.left, y: r.bottom + 2, items: addItems() });
          }}
          aria-haspopup="menu"
        >
          Add
        </Button>
        {undoCount > 0 && (
          <Button icon="refresh" onClick={() => void undo()} title="Undo the last removal (Ctrl+Z)">
            Undo remove{undoCount > 1 ? ` (${undoCount})` : ''}
          </Button>
        )}
        <div className="grow" style={{ maxWidth: 320, position: 'relative' }}>
          <input
            ref={searchRef}
            className="input"
            type="search"
            placeholder="Search this workspace (Ctrl+F)"
            aria-label="Search workspace"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <select
          className="select"
          style={{ width: 118 }}
          aria-label="Filter by type"
          value={kind}
          onChange={(e) => setKind(e.target.value)}
        >
          <option value="">All types</option>
          <option value="file">Files</option>
          <option value="folder">Folders</option>
          <option value="note">Notes</option>
          <option value="link">Links</option>
        </select>
        <select
          className="select"
          style={{ width: 128 }}
          aria-label="Sort by"
          value={sort.key}
          onChange={(e) => setSort({ key: e.target.value, desc: false })}
        >
          <option value="name">Name</option>
          <option value="kind">Type</option>
          <option value="size">Size</option>
          <option value="modified">Modified</option>
          <option value="manual">Manual order</option>
        </select>
        <Button
          aria-pressed={view === 'list'}
          icon="list"
          className="icon"
          aria-label="List view"
          title="List view"
          onClick={() => setView('list')}
        />
        <Button
          aria-pressed={view === 'grid'}
          icon="grid"
          className="icon"
          aria-label="Grid view"
          title="Grid view"
          onClick={() => setView('grid')}
        />
        <div className="grow" />
        <Button variant="primary" icon="upload" onClick={() => setDialog('export')}>
          Export package
        </Button>
      </div>

      <div className="panels">
        <aside className="panel left" aria-label="Navigation">
          <Tree
            entries={entries}
            current={folder}
            rootLabel={ws.name}
            onSelect={(f) => {
              setFolder(f);
              setSearch('');
              setKind('');
              setTag('');
              setSelected(new Set());
              setTab('items');
            }}
            onDropIds={move}
          />
          {tags.length > 0 && (
            <>
              <div className="panel-title" style={{ marginTop: 12 }}>
                Tags
              </div>
              {tags.map(([t, n]) => (
                <button
                  key={t}
                  className="tree-item"
                  aria-selected={tag === t}
                  aria-pressed={tag === t}
                  onClick={() => {
                    setTag(tag === t ? '' : t);
                    setTab('items');
                  }}
                >
                  <Icon name="tag" size={13} />
                  <span className="grow ellipsis">{t}</span>
                  <span className="faint small">{n}</span>
                </button>
              ))}
            </>
          )}
        </aside>

        <section
          className="panel center"
          ref={centerRef}
          aria-label="Contents"
          onContextMenu={(e) => tab === 'items' && showContext(e, null)}
          onClick={(e) => {
            if (e.target === e.currentTarget) setSelected(new Set());
          }}
        >
          <div className="tabs" role="tablist" style={{ padding: '0 14px', background: 'var(--surface)' }}>
            <button
              role="tab"
              className="tab"
              aria-selected={tab === 'items'}
              onClick={() => setTab('items')}
            >
              Items
            </button>
            <button role="tab" className="tab" aria-selected={tab === 'refs'} onClick={() => setTab('refs')}>
              References ({refs.length})
            </button>
            <button
              role="tab"
              className="tab"
              aria-selected={tab === 'health'}
              onClick={() => setTab('health')}
            >
              Health{problemCount > 0 ? ` (${problemCount})` : ''}
            </button>
          </div>
          {tab === 'items' && (
            <>
              <div className="crumbs" aria-label="Location">
                {filtering ? (
                  <span>
                    {plural(items.length, 'result')}
                    {search && <> for “{search}”</>}
                    {tag && <> tagged “{tag}”</>}
                  </span>
                ) : (
                  <>
                    <button onClick={() => setFolder(null)}>{ws.name}</button>
                    {crumbs.map((c) => (
                      <span key={c.id} className="row" style={{ gap: 2 }}>
                        <Icon name="right" size={11} />
                        <button onClick={() => setFolder(c.id)}>{c.name}</button>
                      </span>
                    ))}
                  </>
                )}
                {filtering && (
                  <Button
                    variant="ghost"
                    onClick={() => {
                      setSearch('');
                      setKind('');
                      setTag('');
                    }}
                  >
                    Clear search
                  </Button>
                )}
              </div>
              {problemCount > 0 && (
                <div style={{ padding: '0 14px 8px' }}>
                  <div className="banner warn" role="status">
                    <Icon name="alert" />
                    <div className="grow">
                      {plural(problemCount, 'source file')} {problemCount === 1 ? 'is' : 'are'} missing or
                      changed.
                    </div>
                    <Button onClick={() => setTab('health')}>Review</Button>
                  </div>
                </div>
              )}
              <div
                className="content-scroll"
                tabIndex={0}
                aria-label="Items"
                onClick={(e) => {
                  if (e.target === e.currentTarget) setSelected(new Set());
                }}
              >
                {items.length === 0 ? (
                  filtering ? (
                    <EmptyState icon="search" title="Nothing found">
                      No items match your search or filters.
                    </EmptyState>
                  ) : (
                    <EmptyState
                      icon="upload"
                      title={folder ? 'This folder is empty' : 'Add your first items'}
                      action={
                        <div className="row">
                          <Button variant="primary" icon="file" onClick={pickFiles}>
                            Add files…
                          </Button>
                          <Button icon="folder" onClick={pickFolder}>
                            Add a folder…
                          </Button>
                        </div>
                      }
                    >
                      Drag files or folders here from Explorer, or use the buttons. Files stay where they are
                      — Parcel only records them until you export.
                    </EmptyState>
                  )
                ) : view === 'list' ? (
                  <ListView {...viewProps} />
                ) : (
                  <GridView {...viewProps} />
                )}
              </div>
            </>
          )}
          {tab === 'refs' && (
            <References wsId={id} refs={refs} entries={entries} paths={paths} refresh={refresh} />
          )}
          {tab === 'health' && (
            <Health
              ws={ws}
              checks={checks}
              onCheck={checkSources}
              refresh={refresh}
              onRemoved={announceRemoval}
              select={(ids) => {
                setSelected(new Set(ids));
                setTab('items');
              }}
            />
          )}
          <div className="statusbar" role="status">
            <span>{plural(items.length, 'item')}</span>
            {sel.length > 0 && <span>{sel.length} selected</span>}
            <span>{formatBytes(totalSize)}</span>
            <span className="grow" />
            <span>{ws.revision > 0 ? `Revision ${ws.revision}` : 'Not exported'}</span>
            <span>Saved automatically</span>
          </div>
        </section>

        <aside className="panel right" aria-label="Inspector">
          <Inspector
            ws={ws}
            entries={entries}
            selection={sel}
            paths={paths}
            status={status}
            stats={stats}
            deps={deps}
            refresh={refresh}
            onEditNote={(e) => {
              setNoteTarget(e);
              setDialog('note');
            }}
            onEditWsNotes={() => setDialog('wsnotes')}
            onRename={rename}
            onEditLink={(e) => {
              setNoteTarget(e);
              setDialog('editlink');
            }}
            onBulk={bulk}
          />
        </aside>
      </div>
      <span className="sr-only" aria-live="polite">
        {sel.length ? `${sel.length} selected` : ''}
      </span>

      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
      {dialog === 'note' && noteTarget && (
        <NoteEditor
          title={noteTarget.name}
          initial={byId.get(noteTarget.id)?.content ?? noteTarget.content ?? ''}
          onSave={async (t) => {
            await api.updateEntry(noteTarget.id, { content: t });
            await refresh();
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === 'wsnotes' && (
        <NoteEditor
          title="Workspace notes"
          initial={ws.notes}
          onSave={async (t) => {
            await api.updateWorkspace(id, { notes: t });
            await refresh();
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === 'link' && (
        <LinkDialog
          onClose={() => setDialog(null)}
          onSubmit={async (n, u, d) => {
            await api.createLink(id, folder, n, u, d);
            setDialog(null);
            await refresh();
          }}
        />
      )}
      {dialog === 'editlink' && noteTarget && (
        <LinkDialog
          initial={noteTarget}
          onClose={() => setDialog(null)}
          onSubmit={async (n, u, d) => {
            if (n !== noteTarget.name) await api.renameEntry(noteTarget.id, n);
            await api.updateEntry(noteTarget.id, { url: u, description: d });
            setDialog(null);
            await refresh();
          }}
        />
      )}
      {dialog === 'move' && (
        <MoveDialog
          entries={entries}
          movingIds={sel.map((e) => e.id)}
          onClose={() => setDialog(null)}
          onPick={async (p) => {
            setDialog(null);
            await move(
              sel.map((e) => e.id),
              p,
            );
          }}
        />
      )}
      {dialog === 'export' && (
        <ExportDialog
          ws={ws}
          profiles={profiles}
          onClose={() => setDialog(null)}
          onDone={async (o) => {
            setDialog(null);
            toast(exportSummary(o), 'success', {
              label: 'Show in folder',
              onClick: () => api.revealOutput(o.path).catch((e) => reportError(e, 'Could not show the file')),
            });
            o.warnings.forEach((w) => toast(w, 'warn'));
            await refresh();
            await checkSources(false);
          }}
        />
      )}
    </div>
  );
}
