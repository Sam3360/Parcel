import { useEffect, useState } from 'react';
import { FileIcon } from '../../components/FileIcon';
import { PreviewPane } from '../../components/PreviewPane';
import { Button, IconButton, Badge } from '../../components/ui';
import { formatBytes, formatDateTime, plural } from '../../lib/format';
import { useApp } from '../../hooks/app';
import type { DepRow, Entry, SourceState, Workspace, WorkspaceStats } from '../../lib/types';
import { StatusBadge } from './EntryViews';
import { useOpenLink } from './dialogs';

interface Props {
  ws: Workspace;
  entries: Entry[];
  selection: Entry[];
  paths: Map<string, string>;
  status: Map<string, SourceState>;
  stats: WorkspaceStats | null;
  deps: DepRow[];
  refresh: () => void;
  onEditNote: (e: Entry) => void;
  onEditWsNotes: () => void;
  onRename: (e: Entry) => void;
  onEditLink: (e: Entry) => void;
  onBulk: (action: 'tag' | 'move' | 'remove') => void;
}

export function Inspector(p: Props) {
  if (p.selection.length === 0) return <WorkspaceInfo {...p} />;
  if (p.selection.length > 1) {
    const size = p.selection.reduce((n, e) => n + (e.kind === 'file' || e.kind === 'note' ? e.size : 0), 0);
    return (
      <div className="inspector" aria-label="Inspector">
        <h2>{p.selection.length} items selected</h2>
        <p className="muted">{formatBytes(size)} in total</p>
        <div className="stack">
          <Button icon="tag" onClick={() => p.onBulk('tag')}>
            Add tag…
          </Button>
          <Button icon="folder" onClick={() => p.onBulk('move')}>
            Move to…
          </Button>
          <Button icon="trash" className="danger" onClick={() => p.onBulk('remove')}>
            Remove from workspace…
          </Button>
        </div>
        <p className="muted small">
          Removing only takes items out of this workspace. Original files are never deleted.
        </p>
      </div>
    );
  }
  return <EntryInfo key={p.selection[0].id} {...p} entry={p.selection[0]} />;
}

function Tags({ tags, onChange }: { tags: string[]; onChange: (t: string[]) => void }) {
  const [v, setV] = useState('');
  const add = () => {
    const t = v.trim();
    if (t && !tags.some((x) => x.toLowerCase() === t.toLowerCase())) onChange([...tags, t]);
    setV('');
  };
  return (
    <div className="stack" style={{ gap: 6 }}>
      <div className="label">Tags</div>
      <div className="row wrap">
        {tags.map((t) => (
          <Badge key={t} tone="accent">
            {t}
            <button
              aria-label={`Remove tag ${t}`}
              className="btn ghost"
              style={{ height: 16, width: 16, padding: 0, justifyContent: 'center' }}
              onClick={() => onChange(tags.filter((x) => x !== t))}
            >
              ×
            </button>
          </Badge>
        ))}
      </div>
      <input
        className="input"
        aria-label="Add tag"
        placeholder="Add a tag and press Enter"
        value={v}
        onChange={(e) => setV(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            add();
          }
        }}
      />
    </div>
  );
}

function Metadata({
  value,
  onChange,
}: {
  value: Record<string, string>;
  onChange: (m: Record<string, string>) => void;
}) {
  const [k, setK] = useState(''),
    [v, setV] = useState('');
  const add = () => {
    if (k.trim()) {
      onChange({ ...value, [k.trim()]: v });
      setK('');
      setV('');
    }
  };
  return (
    <div className="stack" style={{ gap: 6 }}>
      <div className="label">Custom metadata</div>
      {Object.entries(value)
        .filter(([key]) => !key.startsWith('parcel.'))
        .map(([key, val]) => (
          <div key={key} className="row">
            <span className="muted small" style={{ width: 90 }} title={key}>
              {key}
            </span>
            <input
              className="input grow"
              aria-label={`Value of ${key}`}
              defaultValue={val}
              onBlur={(e) => e.target.value !== val && onChange({ ...value, [key]: e.target.value })}
            />
            <IconButton
              icon="x"
              label={`Remove ${key}`}
              onClick={() => {
                const n = { ...value };
                delete n[key];
                onChange(n);
              }}
            />
          </div>
        ))}
      <div className="row">
        <input
          className="input"
          style={{ width: 90 }}
          aria-label="Metadata key"
          placeholder="Key"
          value={k}
          onChange={(e) => setK(e.target.value)}
        />
        <input
          className="input grow"
          aria-label="Metadata value"
          placeholder="Value"
          value={v}
          onChange={(e) => setV(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && add()}
        />
        <Button onClick={add} disabled={!k.trim()} aria-label="Add metadata">
          Add
        </Button>
      </div>
    </div>
  );
}

function EntryInfo(p: Props & { entry: Entry }) {
  const { api, reportError } = useApp();
  const { openLink } = useOpenLink();
  const e = p.entry;
  const [desc, setDesc] = useState(e.description);
  useEffect(() => setDesc(e.description), [e.description]);
  const upd = async (u: Parameters<typeof api.updateEntry>[1]) => {
    try {
      await api.updateEntry(e.id, u);
      p.refresh();
    } catch (err) {
      reportError(err, 'Could not save');
    }
  };
  const myDeps = p.deps.filter((d) => d.from_entry === e.id);
  const [dep, setDep] = useState({ kind: 'entry' as 'entry' | 'external', target: '' });
  const addDep = async () => {
    if (!dep.target.trim()) return;
    try {
      await api.addDependency(p.ws.id, e.id, dep.kind, dep.target.trim(), '', true);
      setDep({ ...dep, target: '' });
      p.refresh();
    } catch (err) {
      reportError(err, 'Could not add dependency');
    }
  };
  const st = p.status.get(e.id);
  const others = p.entries.filter((x) => x.id !== e.id && x.kind !== 'folder');
  return (
    <div className="inspector" aria-label="Inspector">
      <div className="row">
        <FileIcon kind={e.kind} name={e.name} mime={e.mime} size={22} />
        <h2 className="grow ellipsis" title={e.name}>
          {e.name}
        </h2>
        <IconButton icon="edit" label="Rename" onClick={() => p.onRename(e)} />
      </div>
      {st && st.state !== 'unchanged' && (
        <div>
          <StatusBadge s={st} />
          {st.state !== 'missing' && 'reason' in st && <span className="muted small"> {st.reason}</span>}
        </div>
      )}
      {(e.kind === 'file' || e.kind === 'note') && (
        <PreviewPane
          load={() => api.previewEntry(e.id)}
          deps={[e.id, e.sha256, e.content]}
          onLink={openLink}
        />
      )}
      <div className="row wrap">
        {e.kind === 'note' && (
          <Button icon="edit" onClick={() => p.onEditNote(e)}>
            Edit note
          </Button>
        )}
        {e.kind === 'link' && (
          <>
            <Button icon="external" onClick={() => e.url && openLink(e.url)}>
              Open link
            </Button>
            <Button icon="edit" onClick={() => p.onEditLink(e)}>
              Edit
            </Button>
          </>
        )}
      </div>
      <dl className="kv">
        <dt>Path</dt>
        <dd>{p.paths.get(e.id)}</dd>
        <dt>Type</dt>
        <dd>{e.kind === 'file' ? (e.mime ?? 'Unknown') : e.kind}</dd>
        {(e.kind === 'file' || e.kind === 'note') && (
          <>
            <dt>Size</dt>
            <dd>{formatBytes(e.size)}</dd>
          </>
        )}
        {e.url && (
          <>
            <dt>URL</dt>
            <dd className="mono">{e.url}</dd>
          </>
        )}
        {e.sha256 && (
          <>
            <dt>SHA-256</dt>
            <dd className="mono" style={{ fontSize: 11 }}>
              {e.sha256}
            </dd>
          </>
        )}
        {e.kind === 'file' && (
          <>
            <dt>Source</dt>
            <dd className="mono" style={{ fontSize: 11 }}>
              {e.source_path ?? '—'}
            </dd>
            <dt>Origin</dt>
            <dd>
              {e.origin === 'imported'
                ? 'Imported from a package (kept by Parcel)'
                : 'Linked — stays where it is'}
            </dd>
          </>
        )}
        <dt>Added</dt>
        <dd>{formatDateTime(e.created_at)}</dd>
        <dt>Changed</dt>
        <dd>{formatDateTime(e.modified_at)}</dd>
      </dl>
      <div className="field">
        <label htmlFor="insp-desc">Description</label>
        <textarea
          id="insp-desc"
          className="textarea"
          value={desc}
          onChange={(ev) => setDesc(ev.target.value)}
          onBlur={() => desc !== e.description && upd({ description: desc })}
          placeholder="What is this and why is it here?"
        />
      </div>
      <Tags tags={e.tags} onChange={(tags) => upd({ tags })} />
      <Metadata value={e.metadata} onChange={(metadata) => upd({ metadata })} />
      {e.kind === 'note' && (
        <div className="field">
          <label htmlFor="insp-att">Attached to</label>
          <select
            id="insp-att"
            className="select"
            value={e.attached_to ?? ''}
            onChange={(ev) => upd({ attached_to: ev.target.value || null })}
          >
            <option value="">Whole workspace</option>
            {others
              .filter((x) => x.kind !== 'note')
              .map((x) => (
                <option key={x.id} value={x.id}>
                  {p.paths.get(x.id)}
                </option>
              ))}
          </select>
        </div>
      )}
      {e.kind !== 'folder' && (
        <div className="stack" style={{ gap: 6 }}>
          <div className="label">Dependencies</div>
          {myDeps.length === 0 && <span className="muted small">None declared.</span>}
          {myDeps.map((d) => (
            <div key={d.id} className="row">
              <Badge>{d.kind}</Badge>
              <span className="grow ellipsis small">
                {d.kind === 'entry' ? (p.paths.get(d.target) ?? d.target) : d.target}
              </span>
              <IconButton
                icon="x"
                label="Remove dependency"
                onClick={async () => {
                  await api.deleteDependency(d.id);
                  p.refresh();
                }}
              />
            </div>
          ))}
          <div className="row">
            <select
              className="select"
              style={{ width: 100 }}
              aria-label="Dependency type"
              value={dep.kind}
              onChange={(ev) => setDep({ kind: ev.target.value as 'entry', target: '' })}
            >
              <option value="entry">Item</option>
              <option value="external">External</option>
            </select>
            {dep.kind === 'entry' ? (
              <select
                className="select grow"
                aria-label="Dependency target"
                value={dep.target}
                onChange={(ev) => setDep({ ...dep, target: ev.target.value })}
              >
                <option value="">Choose…</option>
                {others.map((x) => (
                  <option key={x.id} value={x.id}>
                    {p.paths.get(x.id)}
                  </option>
                ))}
              </select>
            ) : (
              <input
                className="input grow"
                aria-label="Dependency description"
                placeholder="e.g. Python 3.12"
                value={dep.target}
                onChange={(ev) => setDep({ ...dep, target: ev.target.value })}
              />
            )}
            <Button onClick={addDep} disabled={!dep.target.trim()} aria-label="Add dependency">
              Add
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

function WorkspaceInfo(p: Props) {
  const { api, reportError } = useApp();
  const [desc, setDesc] = useState(p.ws.description);
  useEffect(() => setDesc(p.ws.description), [p.ws.description]);
  const s = p.stats;
  const save = async (u: Parameters<typeof api.updateWorkspace>[1]) => {
    try {
      await api.updateWorkspace(p.ws.id, u);
      p.refresh();
    } catch (e) {
      reportError(e);
    }
  };
  return (
    <div className="inspector" aria-label="Workspace overview">
      <h2>{p.ws.name}</h2>
      <div className="field">
        <label htmlFor="ws-desc-i">Description</label>
        <textarea
          id="ws-desc-i"
          className="textarea"
          value={desc}
          onChange={(e) => setDesc(e.target.value)}
          onBlur={() => desc !== p.ws.description && save({ description: desc })}
          placeholder="Describe this workspace"
        />
      </div>
      {s && (
        <dl className="kv" aria-label="Workspace statistics">
          <dt>Files</dt>
          <dd>{s.files}</dd>
          <dt>Folders</dt>
          <dd>{s.folders}</dd>
          <dt>Notes</dt>
          <dd>{s.notes}</dd>
          <dt>Links</dt>
          <dd>{s.links}</dd>
          <dt>References</dt>
          <dd>{s.references}</dd>
          <dt>Total size</dt>
          <dd>{formatBytes(s.total_size)}</dd>
          <dt>Revision</dt>
          <dd>{p.ws.revision === 0 ? 'Not exported yet' : p.ws.revision}</dd>
        </dl>
      )}
      <div className="stack" style={{ gap: 6 }}>
        <div className="label">Workspace notes</div>
        <p className="muted small">
          {p.ws.notes.trim()
            ? `${p.ws.notes.trim().split(/\s+/).length} words`
            : 'No notes yet. They are included in the package.'}
        </p>
        <Button icon="note" onClick={p.onEditWsNotes}>
          Edit workspace notes
        </Button>
      </div>
      {Object.keys(p.ws.metadata).length > 0 && (
        <Metadata value={p.ws.metadata} onChange={(metadata) => save({ metadata })} />
      )}
      {s && s.by_extension.length > 0 && (
        <div>
          <div className="label">Common types</div>
          <p className="muted small">{s.by_extension.map(([x, n]) => `.${x} ×${n}`).join('  ·  ')}</p>
        </div>
      )}
      {s && s.largest.length > 0 && (
        <div>
          <div className="label">Largest files</div>
          {s.largest.map(([n, sz]) => (
            <div key={n} className="row small">
              <span className="grow ellipsis">{n}</span>
              <span className="muted">{formatBytes(sz)}</span>
            </div>
          ))}
        </div>
      )}
      <p className="muted small">{plural(p.entries.length, 'item')} · Select an item to inspect it.</p>
    </div>
  );
}
