import { useEffect, useMemo, useRef, useState } from 'react';
import { Dialog } from '../../components/Dialog';
import { Banner, Button, ErrorText, Field } from '../../components/ui';
import { Markdown } from '../../lib/markdown';
import { describeError, errorCode } from '../../lib/api';
import { formatBytes, plural } from '../../lib/format';
import { useApp } from '../../hooks/app';
import type {
  Entry,
  ExportOutcome,
  ExportProfile,
  IpcError,
  RefInput,
  RefRow,
  Workspace,
} from '../../lib/types';

/** Markdown editor with live preview and debounced autosave. */
export function NoteEditor({
  title,
  initial,
  onSave,
  onClose,
}: {
  title: string;
  initial: string;
  onSave: (text: string) => Promise<void>;
  onClose: () => void;
}) {
  const { openLink } = useOpenLink();
  const [text, setText] = useState(initial);
  const [state, setState] = useState<'saved' | 'saving' | 'dirty' | 'error'>('saved');
  const last = useRef(initial);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const save = async (t: string) => {
    if (t === last.current) return;
    setState('saving');
    try {
      await onSave(t);
      last.current = t;
      setState('saved');
    } catch {
      setState('error');
    }
  };
  useEffect(() => {
    if (text === last.current) return;
    setState('dirty');
    clearTimeout(timer.current);
    timer.current = setTimeout(() => void save(text), 700);
    return () => clearTimeout(timer.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text]);
  const close = async () => {
    clearTimeout(timer.current);
    await save(text);
    onClose();
  };
  return (
    <Dialog
      title={title}
      size="xwide"
      onClose={close}
      dismissible={false}
      footer={
        <>
          <span className="muted small grow" aria-live="polite">
            {
              {
                saved: 'All changes saved',
                saving: 'Saving…',
                dirty: 'Unsaved changes…',
                error: 'Could not save — try again',
              }[state]
            }
          </span>
          <Button variant="primary" onClick={close}>
            Done
          </Button>
        </>
      }
    >
      <div className="note-editor">
        <textarea
          className="textarea"
          aria-label="Markdown source"
          value={text}
          onChange={(e) => setText(e.target.value)}
          spellCheck
        />
        <div className="card card-pad" style={{ overflow: 'auto' }} aria-label="Rendered preview">
          <Markdown source={text} onLink={openLink} />
        </div>
      </div>
    </Dialog>
  );
}

export function useOpenLink() {
  const { api, confirm, reportError } = useApp();
  return {
    openLink: async (url: string) => {
      const ok = await confirm({
        title: 'Open link in your browser?',
        confirmLabel: 'Open',
        message: (
          <>
            <p>Parcel will hand this address to your default browser:</p>
            <p className="mono" style={{ wordBreak: 'break-all' }}>
              {url}
            </p>
          </>
        ),
      });
      if (!ok) return;
      try {
        await api.openExternal(url);
      } catch (e) {
        reportError(e);
      }
    },
  };
}

export function LinkDialog({
  initial,
  onSubmit,
  onClose,
}: {
  initial?: Entry;
  onSubmit: (name: string, url: string, description: string) => Promise<void>;
  onClose: () => void;
}) {
  const [name, setName] = useState(initial?.name ?? '');
  const [url, setUrl] = useState(initial?.url ?? 'https://');
  const [desc, setDesc] = useState(initial?.description ?? '');
  const [error, setError] = useState<string | null>(null);
  const go = async () => {
    try {
      await onSubmit(name.trim(), url.trim(), desc);
    } catch (e) {
      setError(describeError(e));
    }
  };
  return (
    <Dialog
      title={initial ? 'Edit link' : 'Add a link'}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!name.trim() || url.trim().length < 8} onClick={go}>
            {initial ? 'Save' : 'Add link'}
          </Button>
        </>
      }
    >
      <form
        className="stack"
        onSubmit={(e) => {
          e.preventDefault();
          void go();
        }}
      >
        <p className="muted small">
          The address is stored as text only. Parcel never contacts it automatically.
        </p>
        <Field label="Title" htmlFor="lk-name">
          <input id="lk-name" className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="URL" htmlFor="lk-url">
          <input
            id="lk-url"
            className="input"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            spellCheck={false}
          />
        </Field>
        <Field label="Description" htmlFor="lk-desc">
          <textarea
            id="lk-desc"
            className="textarea"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
          />
        </Field>
        <ErrorText error={error} />
      </form>
    </Dialog>
  );
}

export function ReferenceDialog({
  initial,
  entries,
  onSubmit,
  onClose,
}: {
  initial?: RefRow;
  entries: Entry[];
  onSubmit: (r: RefInput) => Promise<void>;
  onClose: () => void;
}) {
  const [title, setTitle] = useState(initial?.title ?? '');
  const [url, setUrl] = useState(initial?.url ?? 'https://');
  const [desc, setDesc] = useState(initial?.description ?? '');
  const [cat, setCat] = useState(initial?.category ?? '');
  const [entry, setEntry] = useState(initial?.entry_id ?? '');
  const [error, setError] = useState<string | null>(null);
  const go = async () => {
    try {
      await onSubmit({ title, url: url.trim(), description: desc, category: cat, entry_id: entry || null });
    } catch (e) {
      setError(describeError(e));
    }
  };
  return (
    <Dialog
      title={initial ? 'Edit reference' : 'Add a reference'}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!title.trim() || url.trim().length < 8} onClick={go}>
            {initial ? 'Save' : 'Add reference'}
          </Button>
        </>
      }
    >
      <form
        className="stack"
        onSubmit={(e) => {
          e.preventDefault();
          void go();
        }}
      >
        <Field label="Title" htmlFor="rf-title">
          <input id="rf-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <Field label="URL" htmlFor="rf-url" hint="http, https, ftp or mailto addresses only.">
          <input
            id="rf-url"
            className="input"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            spellCheck={false}
          />
        </Field>
        <Field label="Category" htmlFor="rf-cat">
          <input
            id="rf-cat"
            className="input"
            value={cat}
            onChange={(e) => setCat(e.target.value)}
            placeholder="e.g. Documentation"
          />
        </Field>
        <Field label="Description" htmlFor="rf-desc">
          <textarea
            id="rf-desc"
            className="textarea"
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
          />
        </Field>
        <Field label="Related item (optional)" htmlFor="rf-entry">
          <select id="rf-entry" className="select" value={entry} onChange={(e) => setEntry(e.target.value)}>
            <option value="">Whole workspace</option>
            {entries
              .filter((e) => e.kind !== 'folder')
              .map((e) => (
                <option key={e.id} value={e.id}>
                  {e.name}
                </option>
              ))}
          </select>
        </Field>
        <ErrorText error={error} />
      </form>
    </Dialog>
  );
}

export function MoveDialog({
  entries,
  movingIds,
  onPick,
  onClose,
}: {
  entries: Entry[];
  movingIds: string[];
  onPick: (parentId: string | null) => void;
  onClose: () => void;
}) {
  const [sel, setSel] = useState<string | null>(null);
  const rows = useMemo(() => {
    const byParent = new Map<string | null, Entry[]>();
    entries
      .filter((e) => e.kind === 'folder')
      .forEach((e) => byParent.set(e.parent_id, [...(byParent.get(e.parent_id) ?? []), e]));
    const out: { e: Entry | null; depth: number; disabled: boolean }[] = [
      { e: null, depth: 0, disabled: false },
    ];
    const walk = (p: string | null, depth: number, blocked: boolean) => {
      (byParent.get(p) ?? [])
        .sort((a, b) => a.name.localeCompare(b.name))
        .forEach((f) => {
          const dis = blocked || movingIds.includes(f.id);
          out.push({ e: f, depth, disabled: dis });
          walk(f.id, depth + 1, dis);
        });
    };
    walk(null, 1, false);
    return out;
  }, [entries, movingIds]);
  return (
    <Dialog
      title={`Move ${plural(movingIds.length, 'item')} to…`}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => onPick(sel)}>
            Move here
          </Button>
        </>
      }
    >
      <div role="listbox" aria-label="Destination folder" className="list">
        {rows.map((r) => (
          <button
            key={r.e?.id ?? 'root'}
            role="option"
            aria-selected={sel === (r.e?.id ?? null)}
            disabled={r.disabled}
            className="list-row"
            style={{
              paddingLeft: 14 + r.depth * 16,
              background: sel === (r.e?.id ?? null) ? 'var(--accent-weak)' : undefined,
            }}
            onClick={() => setSel(r.e?.id ?? null)}
          >
            {r.e ? r.e.name : 'Workspace root'}
          </button>
        ))}
      </div>
    </Dialog>
  );
}

export function ExportDialog({
  ws,
  profiles,
  onClose,
  onDone,
}: {
  ws: Workspace;
  profiles: ExportProfile[];
  onClose: () => void;
  onDone: (o: ExportOutcome) => void;
}) {
  const { api, runJob } = useApp();
  const [profileId, setProfileId] = useState(ws.export_profile_id ?? 'prof-standard');
  const [problems, setProblems] = useState<IpcError['details'] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const profile = profiles.find((p) => p.id === profileId);
  const suggested = () => {
    const pat = profile?.spec.filename_pattern || '{name}';
    return (
      pat
        .replace('{name}', ws.name)
        .replace('{date}', new Date().toISOString().slice(0, 10))
        .replace(/[\\/:*?"<>|]/g, '_') + '.parcel'
    );
  };
  const run = async (accept: boolean, dest?: string) => {
    setError(null);
    setBusy(true);
    try {
      if (profileId !== ws.export_profile_id)
        await api.updateWorkspace(ws.id, { export_profile_id: profileId });
      const target = dest ?? (await api.pickSaveParcel(suggested()));
      if (!target) {
        setBusy(false);
        return;
      }
      // The native save dialog has already asked the user to confirm replacing an existing file.
      const out = await runJob('Exporting package', (id) =>
        api.exportWorkspace(ws.id, target, true, accept, id),
      );
      onDone(out);
    } catch (e) {
      setBusy(false);
      if (errorCode(e) === 'source_problems') setProblems((e as IpcError).details ?? {});
      else if (errorCode(e) === 'cancelled') setError('Export cancelled. No package was written.');
      else setError(describeError(e));
    }
  };
  const missing = problems?.missing ?? [],
    changed = problems?.changed ?? [];
  return (
    <Dialog
      title="Export package"
      onClose={onClose}
      size={problems ? 'wide' : 'normal'}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          {problems ? (
            <>
              {changed.length > 0 && missing.length === 0 && (
                <Button variant="primary" disabled={busy} onClick={() => run(true)}>
                  Accept changes and export
                </Button>
              )}
            </>
          ) : (
            <Button variant="primary" disabled={busy} onClick={() => run(false)}>
              Choose location and export
            </Button>
          )}
        </>
      }
    >
      {problems ? (
        <div className="stack">
          <Banner tone="warn">
            Some source files are not as they were when collected. Nothing has been written. Parcel never
            silently replaces collected content.
          </Banner>
          {missing.length > 0 && (
            <>
              <h3>Missing ({missing.length})</h3>
              <ul>
                {missing.map((m) => (
                  <li key={m.entry_id}>
                    <strong>{m.path}</strong>
                    <div className="muted small mono">{m.source_path}</div>
                  </li>
                ))}
              </ul>
              <p className="muted">
                Relink these files from the Health tab, or remove them from the workspace.
              </p>
            </>
          )}
          {changed.length > 0 && (
            <>
              <h3>Changed ({changed.length})</h3>
              <ul>
                {changed.map((m) => (
                  <li key={m.entry_id}>
                    <strong>{m.path}</strong> — {m.detail}
                  </li>
                ))}
              </ul>
              {missing.length === 0 && (
                <p className="muted">
                  Accepting will package the current versions and record them as the new baseline.
                </p>
              )}
            </>
          )}
        </div>
      ) : (
        <div className="stack">
          <p className="muted">
            Packages “{ws.name}” into a single portable .parcel file. Your original files are only read, never
            modified. Revision {ws.revision + 1} will be created.
          </p>
          <Field label="Export profile" htmlFor="ex-prof" hint={profile?.description}>
            <select
              id="ex-prof"
              className="select"
              value={profileId}
              onChange={(e) => setProfileId(e.target.value)}
            >
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </Field>
          {ws.last_export_path && (
            <p className="muted small">
              Last exported to <span className="mono">{ws.last_export_path}</span>
            </p>
          )}
          <ErrorText error={error} />
        </div>
      )}
    </Dialog>
  );
}

export const exportSummary = (o: ExportOutcome) =>
  `Exported ${plural(o.entry_count, 'entry', 'entries')} (${formatBytes(o.package_size)}) as revision ${o.revision}`;
