import { useMemo, useState } from 'react';
import { Badge, Button, EmptyState, IconButton } from '../../components/ui';
import { useApp } from '../../hooks/app';
import type { Entry, RefRow } from '../../lib/types';
import { ReferenceDialog, useOpenLink } from './dialogs';

export function References({
  wsId,
  refs,
  entries,
  paths,
  refresh,
}: {
  wsId: string;
  refs: RefRow[];
  entries: Entry[];
  paths: Map<string, string>;
  refresh: () => void;
}) {
  const { api, confirm, reportError } = useApp();
  const { openLink } = useOpenLink();
  const [edit, setEdit] = useState<RefRow | 'new' | null>(null);
  const groups = useMemo(() => {
    const m = new Map<string, RefRow[]>();
    refs.forEach((r) =>
      m.set(r.category || 'Uncategorised', [...(m.get(r.category || 'Uncategorised') ?? []), r]),
    );
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [refs]);
  const del = async (r: RefRow) => {
    if (
      !(await confirm({
        title: 'Delete reference?',
        confirmLabel: 'Delete',
        danger: true,
        message: <p>“{r.title}” will be removed from this workspace.</p>,
      }))
    )
      return;
    try {
      await api.deleteReference(r.id);
      refresh();
    } catch (e) {
      reportError(e);
    }
  };
  return (
    <div className="content-scroll" style={{ paddingTop: 14 }}>
      <div className="row" style={{ marginBottom: 12 }}>
        <p className="muted grow">
          Web addresses stored with your workspace. Parcel never loads them; they open in your browser only
          when you choose.
        </p>
        <Button variant="primary" icon="plus" onClick={() => setEdit('new')}>
          Add reference
        </Button>
      </div>
      {refs.length === 0 ? (
        <div className="card">
          <EmptyState icon="link" title="No references yet">
            Add articles, documentation or any link that belongs with this collection.
          </EmptyState>
        </div>
      ) : (
        groups.map(([cat, rows]) => (
          <section key={cat} style={{ marginBottom: 18 }} aria-label={cat}>
            <h3 style={{ marginBottom: 6 }}>{cat}</h3>
            <div className="list">
              {rows.map((r) => (
                <div key={r.id} className="list-row">
                  <div className="grow">
                    <button
                      style={{ all: 'unset', cursor: 'pointer', color: 'var(--accent)', fontWeight: 600 }}
                      onClick={() => openLink(r.url)}
                    >
                      {r.title}
                    </button>
                    <div className="muted small mono ellipsis">{r.url}</div>
                    {r.description && <div className="small">{r.description}</div>}
                  </div>
                  {r.entry_id && <Badge>{paths.get(r.entry_id) ?? 'item'}</Badge>}
                  <IconButton icon="edit" label={`Edit ${r.title}`} onClick={() => setEdit(r)} />
                  <IconButton icon="trash" label={`Delete ${r.title}`} onClick={() => del(r)} />
                </div>
              ))}
            </div>
          </section>
        ))
      )}
      {edit && (
        <ReferenceDialog
          initial={edit === 'new' ? undefined : edit}
          entries={entries}
          onClose={() => setEdit(null)}
          onSubmit={async (input) => {
            if (edit === 'new') await api.addReference(wsId, input);
            else await api.updateReference(edit.id, input);
            setEdit(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}
