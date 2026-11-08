import { useCallback, useEffect, useState } from 'react';
import { Badge, Banner, Button, EmptyState, Loading } from '../../components/ui';
import { StatusBadge } from './EntryViews';
import { formatBytes, formatDateTime, plural } from '../../lib/format';
import { useApp } from '../../hooks/app';
import type { Diagnostic, DuplicateGroup, RevisionRow, SourceCheck, Workspace } from '../../lib/types';

export function Health({
  ws,
  checks,
  onCheck,
  refresh,
  select,
}: {
  ws: Workspace;
  checks: SourceCheck[] | null;
  onCheck: (deep: boolean) => Promise<void>;
  refresh: () => void;
  select: (ids: string[]) => void;
}) {
  const { api, confirm, reportError, toast } = useApp();
  const [dups, setDups] = useState<DuplicateGroup[] | null>(null);
  const [diag, setDiag] = useState<Diagnostic[] | null>(null);
  const [revs, setRevs] = useState<RevisionRow[]>([]);
  const [checking, setChecking] = useState(false);
  const load = useCallback(() => {
    api
      .findDuplicates(ws.id)
      .then(setDups)
      .catch((e) => {
        setDups([]);
        reportError(e);
      });
    api
      .workspaceDiagnostics(ws.id)
      .then(setDiag)
      .catch(() => setDiag([]));
    api
      .listRevisions(ws.id)
      .then(setRevs)
      .catch(() => setRevs([]));
  }, [api, ws.id, reportError]);
  useEffect(load, [load, ws.modified_at]);

  const run = async (deep: boolean) => {
    setChecking(true);
    try {
      await onCheck(deep);
    } finally {
      setChecking(false);
    }
  };
  const relink = async (c: SourceCheck) => {
    const f = await api.pickAnyFile();
    if (!f) return;
    try {
      await api.relinkSource(c.entry_id, f, false);
      toast('Relinked — content matches the collected file', 'success');
      refresh();
      await onCheck(false);
    } catch (e) {
      const ok = await confirm({
        title: 'Content differs',
        confirmLabel: 'Use this file anyway',
        danger: true,
        message: (
          <p>
            The chosen file’s content is different from what was collected. If you continue, it becomes the
            new version for this item.
          </p>
        ),
      });
      if (!ok) return;
      try {
        await api.relinkSource(c.entry_id, f, true);
        refresh();
        await onCheck(false);
      } catch (e2) {
        reportError(e2);
      }
      void e;
    }
  };
  const accept = async (ids: string[]) => {
    try {
      await api.acceptSourceChanges(ids);
      toast('Changes accepted as the new baseline', 'success');
      refresh();
      await onCheck(false);
    } catch (e) {
      reportError(e);
    }
  };
  const removeExtras = async (g: DuplicateGroup) => {
    const extras = g.ids.slice(1);
    if (
      !(await confirm({
        title: 'Remove duplicate copies?',
        confirmLabel: 'Remove copies',
        danger: true,
        message: (
          <>
            <p>
              Keeps “{g.labels[0]}” and removes {plural(extras.length, 'identical copy', 'identical copies')}{' '}
              from this workspace:
            </p>
            <ul>
              {g.labels.slice(1).map((l) => (
                <li key={l}>{l}</li>
              ))}
            </ul>
            <p className="muted">Original files on disk are not touched.</p>
          </>
        ),
      }))
    )
      return;
    try {
      await api.deleteEntries(extras);
      refresh();
      load();
    } catch (e) {
      reportError(e);
    }
  };

  const problems = checks?.filter((c) => c.state !== 'unchanged') ?? [];
  return (
    <div className="content-scroll" style={{ paddingTop: 14 }}>
      <section className="stack" aria-labelledby="h-src">
        <div className="row">
          <h2 id="h-src" className="grow">
            Source files
          </h2>
          <Button icon="refresh" disabled={checking} onClick={() => run(false)}>
            Check sources
          </Button>
          <Button
            disabled={checking}
            onClick={() => run(true)}
            title="Re-reads every file and compares its checksum"
          >
            Deep check
          </Button>
        </div>
        {checks === null ? (
          <p className="muted">Sources have not been checked in this session.</p>
        ) : problems.length === 0 ? (
          <Banner tone="ok">All {plural(checks.length, 'source file')} match what was collected.</Banner>
        ) : (
          <div className="list">
            {problems.map((c) => (
              <div key={c.entry_id} className="list-row">
                <StatusBadge s={c} />
                <button
                  className="grow"
                  style={{ all: 'unset', cursor: 'pointer', flex: 1, minWidth: 0 }}
                  onClick={() => select([c.entry_id])}
                >
                  <div className="ellipsis">{c.path}</div>
                  <div className="muted small mono ellipsis">{c.source_path}</div>
                </button>
                {c.state === 'changed' && <Button onClick={() => accept([c.entry_id])}>Accept change</Button>}
                <Button onClick={() => relink(c)}>Relink…</Button>
              </div>
            ))}
            {problems.filter((c) => c.state === 'changed').length > 1 && (
              <div className="list-row">
                <Button
                  onClick={() => accept(problems.filter((c) => c.state === 'changed').map((c) => c.entry_id))}
                >
                  Accept all changes
                </Button>
              </div>
            )}
          </div>
        )}
      </section>

      <section className="stack section" aria-labelledby="h-dup">
        <h2 id="h-dup">Duplicate content</h2>
        {dups === null ? (
          <Loading />
        ) : dups.length === 0 ? (
          <p className="muted">No identical files found.</p>
        ) : (
          dups.map((g) => (
            <div key={g.sha256} className="card card-pad stack" style={{ gap: 6 }}>
              <div className="row">
                <strong className="grow">
                  {plural(g.ids.length, 'identical file')} · {formatBytes(g.size)} each
                </strong>
                <Badge tone="warn">{formatBytes(g.wasted_bytes)} saved if reduced to one</Badge>
              </div>
              {g.labels.map((l, i) => (
                <div key={l} className="small">
                  {l}
                  {i === 0 && <span className="muted"> (kept)</span>}
                </div>
              ))}
              <div className="row">
                <Button onClick={() => select(g.ids)}>Select all</Button>
                <Button className="danger" onClick={() => removeExtras(g)}>
                  Remove extra copies…
                </Button>
              </div>
            </div>
          ))
        )}
      </section>

      <section className="stack section" aria-labelledby="h-diag">
        <h2 id="h-diag">Dependency checks</h2>
        {diag === null ? (
          <Loading />
        ) : diag.length === 0 ? (
          <p className="muted">No dependency or reference issues.</p>
        ) : (
          diag.map((d, i) => (
            <Banner
              key={i}
              tone={d.severity === 'error' ? 'error' : d.severity === 'warning' ? 'warn' : 'info'}
            >
              {d.message}
            </Banner>
          ))
        )}
      </section>

      <section className="stack section" aria-labelledby="h-rev">
        <h2 id="h-rev">Revisions</h2>
        {revs.length === 0 ? (
          <EmptyState icon="package" title="Not exported yet">
            Each export is recorded as a numbered revision of this workspace.
          </EmptyState>
        ) : (
          <div className="list">
            {revs.map((r) => (
              <div key={r.revision} className="list-row">
                <Badge tone="accent">rev {r.revision}</Badge>
                <div className="grow">
                  <div className="ellipsis mono small">{r.path}</div>
                </div>
                <span className="muted small">
                  {plural(r.entry_count, 'entry', 'entries')} · {formatBytes(r.package_size)}
                </span>
                <span className="faint small">{formatDateTime(r.exported_at)}</span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
