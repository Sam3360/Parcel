import { Fragment, useCallback, useEffect, useMemo, useState } from 'react';
import { useApp } from '../hooks/app';
import { Badge, Banner, Button, EmptyState, ErrorText, Loading } from '../components/ui';
import { Dialog } from '../components/Dialog';
import { FileIcon } from '../components/FileIcon';
import { Icon } from '../components/Icon';
import { PreviewPane } from '../components/PreviewPane';
import { Markdown } from '../lib/markdown';
import { useOpenLink } from '../features/editor/dialogs';
import { SpacerRow, useVirtualRows } from '../lib/virtual';
import { ZipImport } from '../features/ZipImport';
import { describeError, errorCode } from '../lib/api';
import {
  baseName,
  extOf,
  formatBytes,
  formatDateTime,
  parentPath,
  plural,
  relativeTime,
} from '../lib/format';
import type {
  Comparison,
  ConflictPolicy,
  IntegrityReport,
  ManifestEntry,
  PackageInfo,
  RecentPackage,
} from '../lib/types';

export function OpenPackagePage() {
  const { route, navigate, api } = useApp();
  const path = route.page === 'open' ? route.path : undefined;
  if (!path) return <Landing onOpen={(p) => navigate({ page: 'open', path: p })} api={api} />;
  return <Explorer key={path} path={path} onClose={() => navigate({ page: 'open' })} />;
}

function Landing({ onOpen, api }: { onOpen: (p: string) => void; api: ReturnType<typeof useApp>['api'] }) {
  const { reportError } = useApp();
  const [recent, setRecent] = useState<RecentPackage[] | null>(null);
  const [zip, setZip] = useState(false);
  useEffect(() => {
    api
      .listRecentPackages(12)
      .then(setRecent)
      .catch(() => setRecent([]));
  }, [api]);
  const pick = async () => {
    try {
      const p = await api.pickPackage();
      if (p) onOpen(p);
    } catch (e) {
      reportError(e);
    }
  };
  return (
    <div className="page">
      <div className="page-narrow">
        <div className="page-head">
          <div>
            <h1>Open Package</h1>
            <p>Browse, verify and extract any .parcel file. No workspace is needed.</p>
          </div>
          <div className="row">
            <Button icon="download" onClick={() => setZip(true)}>
              Import ZIP…
            </Button>
            <Button variant="primary" icon="package" onClick={pick}>
              Open package…
            </Button>
          </div>
        </div>
        <section aria-labelledby="rp">
          <header className="row" style={{ marginBottom: 8 }}>
            <h2 id="rp" className="grow">
              Recent packages
            </h2>
            {recent && recent.length > 0 && (
              <Button
                variant="ghost"
                onClick={async () => {
                  for (const r of recent) await api.removeRecentPackage(r.path);
                  setRecent([]);
                }}
              >
                Clear list
              </Button>
            )}
          </header>
          {recent === null ? (
            <Loading />
          ) : recent.length === 0 ? (
            <div className="card">
              <EmptyState
                icon="package"
                title="Nothing opened yet"
                action={
                  <Button variant="primary" onClick={pick}>
                    Open a package…
                  </Button>
                }
              >
                Packages you open will appear here for quick access.
              </EmptyState>
            </div>
          ) : (
            <div className="list">
              {recent.map((r) => (
                <button key={r.path} className="list-row" onClick={() => onOpen(r.path)}>
                  <Icon name="package" />
                  <div className="grow">
                    <div className="ellipsis">{r.name}</div>
                    <div className="muted small mono ellipsis">{r.path}</div>
                  </div>
                  <span className="muted small">{formatBytes(r.size)}</span>
                  <span className="faint small">{relativeTime(r.opened_at)}</span>
                </button>
              ))}
            </div>
          )}
        </section>
        {zip && <ZipImport onClose={() => setZip(false)} />}
      </div>
    </div>
  );
}

interface PackageTableProps {
  items: ManifestEntry[];
  search: string;
  sel: Set<string>;
  setSel: React.Dispatch<React.SetStateAction<Set<string>>>;
  setDir: (d: string) => void;
  setSearch: (s: string) => void;
  failures: Map<string, unknown>;
  verified: boolean;
}

/** The entries table lives in its own component so its row window is created and destroyed with the table. */
function PackageTable({
  items,
  search,
  sel,
  setSel,
  setDir,
  setSearch,
  failures,
  verified,
}: PackageTableProps) {
  const v = useVirtualRows(items.length);
  return (
    <table className="table virtual" aria-label="Package entries" aria-rowcount={items.length + 1}>
      <thead>
        <tr>
          <th>Name</th>
          {search && <th>Location</th>}
          <th>Type</th>
          <th>Size</th>
          <th>Status</th>
        </tr>
      </thead>
      <tbody ref={v.tbodyRef}>
        <SpacerRow height={v.topPad} cols={search ? 5 : 4} />
        {items.slice(v.start, v.end).map((e, k) => (
          <tr
            key={e.id}
            data-vrow
            aria-rowindex={v.start + k + 2}
            aria-selected={sel.has(e.id)}
            onClick={(ev) =>
              setSel((s) =>
                ev.ctrlKey || ev.metaKey
                  ? new Set(s.has(e.id) ? [...s].filter((x) => x !== e.id) : [...s, e.id])
                  : new Set([e.id]),
              )
            }
            onDoubleClick={() => {
              if (e.kind === 'folder') {
                setDir(e.path);
                setSearch('');
              }
            }}
          >
            <td>
              <div className="row">
                <FileIcon kind={e.kind} name={baseName(e.path)} mime={e.mime} />
                <span className="ellipsis" style={{ maxWidth: 380 }}>
                  {baseName(e.path)}
                </span>
              </div>
            </td>
            {search && <td className="muted small">{parentPath(e.path) || '/'}</td>}
            <td className="muted">{e.kind === 'file' ? extOf(e.path).toUpperCase() || 'File' : e.kind}</td>
            <td className="muted">{e.kind === 'file' || e.kind === 'note' ? formatBytes(e.size) : ''}</td>
            <td>
              {failures.has(e.id) ? (
                <Badge tone="danger">Corrupted</Badge>
              ) : verified && e.sha256 ? (
                <Badge tone="ok">OK</Badge>
              ) : null}
            </td>
          </tr>
        ))}
        <SpacerRow height={v.bottomPad} cols={search ? 5 : 4} />
      </tbody>
    </table>
  );
}

type Tab = 'contents' | 'notes' | 'refs' | 'checks' | 'compare';

function Explorer({ path, onClose }: { path: string; onClose: () => void }) {
  const { api, runJob, navigate, toast, reportError, confirm } = useApp();
  const { openLink } = useOpenLink();
  const [info, setInfo] = useState<PackageInfo | null>(null);
  const [error, setError] = useState<{ code?: string; message: string } | null>(null);
  const [dir, setDir] = useState('');
  const [search, setSearch] = useState('');
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [tab, setTab] = useState<Tab>('contents');
  const [report, setReport] = useState<IntegrityReport | null>(null);
  const [cmp, setCmp] = useState<{ other: string; result: Comparison } | null>(null);
  const [extract, setExtract] = useState<{ ids: string[] | null } | null>(null);

  useEffect(() => {
    let live = true;
    api
      .openPackage(path)
      .then((i) => live && setInfo(i))
      .catch((e) => live && setError({ code: errorCode(e), message: describeError(e) }));
    return () => {
      live = false;
    };
  }, [api, path]);

  const entries = useMemo(() => info?.manifest.entries ?? [], [info]);
  const byId = useMemo(() => new Map(entries.map((e) => [e.id, e])), [entries]);
  const failures = useMemo(() => new Map((report?.failures ?? []).map((f) => [f.id, f])), [report]);
  const items = useMemo(() => {
    const q = search.trim().toLowerCase();
    const base = q
      ? entries.filter((e) =>
          `${e.path} ${e.description} ${e.tags.join(' ')} ${e.url ?? ''}`.toLowerCase().includes(q),
        )
      : entries.filter((e) => parentPath(e.path) === dir);
    return [...base].sort(
      (a, b) =>
        (a.kind === 'folder' ? 0 : 1) - (b.kind === 'folder' ? 0 : 1) ||
        a.path.localeCompare(b.path, undefined, { numeric: true }),
    );
  }, [entries, dir, search]);
  const folders = useMemo(() => {
    const s = new Set<string>();
    entries.forEach((e) => {
      if (e.kind === 'folder') s.add(e.path);
      let p = parentPath(e.path);
      while (p) {
        s.add(p);
        p = parentPath(p);
      }
    });
    return [...s].sort();
  }, [entries]);
  // Only folders relevant to where you are: top level, and the siblings/children along the current path.
  const outline = useMemo(
    () =>
      folders.filter((f) => {
        const pp = parentPath(f);
        return pp === '' || pp === dir || dir === pp || dir.startsWith(pp + '/');
      }),
    [folders, dir],
  );
  const selected = [...sel].map((i) => byId.get(i)).filter(Boolean) as ManifestEntry[];
  const one = selected.length === 1 ? selected[0] : null;

  const verify = useCallback(async () => {
    try {
      const r = await runJob('Verifying package integrity', (id) => api.verifyPackage(path, id));
      setReport(r);
      setTab('checks');
      toast(
        r.ok
          ? 'Integrity check passed'
          : `Integrity check found ${r.failures.length + r.metadata_failures.length} problem(s)`,
        r.ok ? 'success' : 'error',
      );
    } catch (e) {
      if (errorCode(e) === 'cancelled') toast('Verification cancelled');
      else reportError(e, 'Verification failed');
    }
  }, [api, path, runJob, toast, reportError]);

  const doExtract = async (ids: string[] | null, policy: ConflictPolicy) => {
    const dest = await api.pickDirectory('Choose where to extract');
    if (!dest) return false;
    try {
      const r = await runJob('Extracting', (id) => api.extractPackage(path, dest, ids, policy, id));
      toast(
        `Extracted ${plural(r.written.length, 'file')} (${formatBytes(r.bytes_written)})${r.skipped.length ? `, ${r.skipped.length} skipped` : ''}`,
        'success',
        {
          label: 'Show in folder',
          onClick: () => api.revealOutput(dest).catch((e) => reportError(e, 'Could not show the folder')),
        },
      );
      return true;
    } catch (e) {
      if (errorCode(e) === 'already_exists')
        toast(
          `Nothing was written: ${describeError(e)}. Choose “Keep both” or “Skip existing” to continue.`,
          'error',
        );
      else if (errorCode(e) === 'cancelled') toast('Extraction cancelled');
      else reportError(e, 'Extraction failed');
      return false;
    }
  };
  const importAsWorkspace = async () => {
    if (
      !(await confirm({
        title: 'Import as a workspace?',
        confirmLabel: 'Import',
        message: (
          <p>
            Creates an editable workspace from this package. Its files are copied into Parcel’s own storage
            after being verified; the .parcel file is not changed.
          </p>
        ),
      }))
    )
      return;
    try {
      const w = await runJob('Importing package', (id) => api.importPackage(path, id));
      toast(`Imported “${w.name}”`, 'success');
      navigate({ page: 'workspaces', workspaceId: w.id });
    } catch (e) {
      if (errorCode(e) !== 'cancelled') reportError(e, 'Import failed');
    }
  };
  const compare = async () => {
    const other = await api.pickPackage();
    if (!other) return;
    try {
      setCmp({ other, result: await api.comparePackages(other, path) });
      setTab('compare');
    } catch (e) {
      reportError(e, 'Could not compare');
    }
  };

  if (error)
    return (
      <div className="page">
        <div className="page-narrow">
          <EmptyState
            icon="shield"
            title="This package could not be opened"
            action={
              <div className="row">
                <Button onClick={onClose}>Back</Button>
              </div>
            }
          >
            <ErrorText error={error.message} />
            {[
              'unsafe_path',
              'limit_exceeded',
              'not_a_package',
              'invalid_manifest',
              'unsupported_version',
            ].includes(error.code ?? '')
              ? 'Parcel refused to open it to keep your computer safe. Nothing was extracted or run.'
              : 'Check that the file exists and is not in use by another program.'}
          </EmptyState>
        </div>
      </div>
    );
  if (!info) return <Loading label="Opening package…" />;
  const m = info.manifest;
  const crumbs = dir ? dir.split('/') : [];
  const diagErrors = info.diagnostics.filter((d) => d.severity !== 'info').length;

  return (
    <div className="editor">
      <div className="toolbar">
        <Button variant="ghost" className="icon" aria-label="Close package" onClick={onClose}>
          <Icon name="x" />
        </Button>
        <div className="grow" style={{ minWidth: 200 }}>
          <div className="row">
            <strong className="ellipsis" style={{ fontSize: 15 }}>
              {m.name}
            </strong>
            <Badge>rev {m.revision}</Badge>
            <Badge>{plural(entries.length, 'entry', 'entries')}</Badge>
            <Badge>{formatBytes(info.file_size)}</Badge>
            {report && (
              <Badge tone={report.ok ? 'ok' : 'danger'}>
                {report.ok ? 'Verified' : 'Integrity problems'}
              </Badge>
            )}
          </div>
          <div className="muted small ellipsis">{m.description || path}</div>
        </div>
        <Button icon="shield" onClick={verify}>
          Verify integrity
        </Button>
        <Button icon="download" onClick={() => setExtract({ ids: null })}>
          Extract all…
        </Button>
        <Button icon="download" disabled={sel.size === 0} onClick={() => setExtract({ ids: [...sel] })}>
          Extract selected…
        </Button>
        <Button icon="layers" onClick={importAsWorkspace}>
          Import as workspace
        </Button>
        <Button icon="compare" onClick={compare}>
          Compare…
        </Button>
      </div>
      <div className="panels">
        <aside className="panel left" aria-label="Folders">
          <div className="panel-title">Folders</div>
          <button
            className="tree-item"
            aria-selected={dir === '' && !search}
            onClick={() => {
              setDir('');
              setSearch('');
              setTab('contents');
            }}
          >
            <Icon name="package" /> <span className="ellipsis">{m.name}</span>
          </button>
          {outline.map((f) => (
            <button
              key={f}
              className="tree-item"
              aria-selected={dir === f}
              style={{ paddingLeft: 8 + f.split('/').length * 12 }}
              onClick={() => {
                setDir(f);
                setSearch('');
                setTab('contents');
              }}
            >
              <Icon name="folder" className="ficon folder" />
              <span className="ellipsis">{baseName(f)}</span>
            </button>
          ))}
        </aside>
        <section className="panel center" aria-label="Package contents">
          <div className="tabs" role="tablist" style={{ padding: '0 14px', background: 'var(--surface)' }}>
            {(
              [
                ['contents', 'Contents'],
                ['notes', 'Notes'],
                ['refs', `References (${m.references.length})`],
                ['checks', `Checks${diagErrors || (report && !report.ok) ? ' ⚠' : ''}`],
                ['compare', 'Compare'],
              ] as [Tab, string][]
            ).map(([k, l]) => (
              <button key={k} role="tab" className="tab" aria-selected={tab === k} onClick={() => setTab(k)}>
                {l}
              </button>
            ))}
          </div>
          {tab === 'contents' && (
            <>
              <div className="row" style={{ padding: '8px 14px' }}>
                <input
                  className="input"
                  style={{ maxWidth: 320 }}
                  type="search"
                  aria-label="Search package"
                  placeholder="Search this package"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                />
                <div className="crumbs" style={{ padding: 0 }}>
                  <button onClick={() => setDir('')}>{m.name}</button>
                  {crumbs.map((c, i) => (
                    <span key={i} className="row" style={{ gap: 2 }}>
                      <Icon name="right" size={11} />
                      <button onClick={() => setDir(crumbs.slice(0, i + 1).join('/'))}>{c}</button>
                    </span>
                  ))}
                </div>
              </div>
              <div className="content-scroll">
                {items.length === 0 ? (
                  <EmptyState
                    icon={search ? 'search' : 'folder'}
                    title={search ? 'No matches' : 'This folder is empty'}
                  />
                ) : (
                  <PackageTable
                    items={items}
                    search={search}
                    sel={sel}
                    setSel={setSel}
                    setDir={setDir}
                    setSearch={setSearch}
                    failures={failures}
                    verified={!!report}
                  />
                )}
              </div>
            </>
          )}
          {tab === 'notes' && (
            <div className="content-scroll" style={{ paddingTop: 14 }}>
              {info.notes.trim() ? (
                <div className="card card-pad">
                  <Markdown source={info.notes} onLink={openLink} />
                </div>
              ) : (
                <EmptyState icon="note" title="No workspace notes">
                  This package has no package-level notes. Individual notes appear in Contents.
                </EmptyState>
              )}
            </div>
          )}
          {tab === 'refs' && (
            <div className="content-scroll" style={{ paddingTop: 14 }}>
              {m.references.length === 0 ? (
                <EmptyState icon="link" title="No references" />
              ) : (
                <div className="list">
                  {m.references.map((r) => (
                    <div key={r.id} className="list-row">
                      <div className="grow">
                        <button
                          style={{ all: 'unset', cursor: 'pointer', color: 'var(--accent)', fontWeight: 600 }}
                          onClick={() => openLink(r.url)}
                        >
                          {r.title}
                        </button>
                        {r.category && (
                          <>
                            {' '}
                            <Badge>{r.category}</Badge>
                          </>
                        )}
                        <div className="muted small mono ellipsis">{r.url}</div>
                        {r.description && <div className="small">{r.description}</div>}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
          {tab === 'checks' && <Checks info={info} report={report} onVerify={verify} />}
          {tab === 'compare' && <CompareView cmp={cmp} current={path} onPick={compare} />}
          <div className="statusbar">
            <span>{plural(items.length, 'item')}</span>
            {sel.size > 0 && <span>{sel.size} selected</span>}
            <span className="grow" />
            <span className="mono">{baseName(path)}</span>
            <span>Format {m.format_version}</span>
          </div>
        </section>
        <aside className="panel right" aria-label="Details">
          {one ? (
            <div className="inspector">
              <div className="row">
                <FileIcon kind={one.kind} name={baseName(one.path)} size={22} />
                <h2 className="ellipsis" title={one.path}>
                  {baseName(one.path)}
                </h2>
              </div>
              {(one.kind === 'file' || one.kind === 'note') && (
                <PreviewPane
                  load={() => api.previewPackageEntry(path, one.id)}
                  deps={[path, one.id]}
                  onLink={openLink}
                />
              )}
              {one.kind === 'link' && one.url && (
                <Button icon="external" onClick={() => openLink(one.url!)}>
                  Open link
                </Button>
              )}
              <dl className="kv">
                <dt>Path</dt>
                <dd>{one.path}</dd>
                <dt>Type</dt>
                <dd>{one.mime ?? one.kind}</dd>
                {one.kind !== 'folder' && one.kind !== 'link' && (
                  <>
                    <dt>Size</dt>
                    <dd>{formatBytes(one.size)}</dd>
                  </>
                )}
                {one.sha256 && (
                  <>
                    <dt>SHA-256</dt>
                    <dd className="mono" style={{ fontSize: 11 }}>
                      {one.sha256}
                    </dd>
                  </>
                )}
                {one.url && (
                  <>
                    <dt>URL</dt>
                    <dd className="mono">{one.url}</dd>
                  </>
                )}
                {one.modified_at && (
                  <>
                    <dt>Modified</dt>
                    <dd>{formatDateTime(one.modified_at)}</dd>
                  </>
                )}
              </dl>
              {one.description && (
                <div>
                  <div className="label">Description</div>
                  <p>{one.description}</p>
                </div>
              )}
              {one.tags.length > 0 && (
                <div className="row wrap">
                  {one.tags.map((t) => (
                    <Badge key={t} tone="accent">
                      {t}
                    </Badge>
                  ))}
                </div>
              )}
              {Object.entries(one.metadata).filter(([k]) => !k.startsWith('parcel.')).length > 0 && (
                <dl className="kv">
                  {Object.entries(one.metadata)
                    .filter(([k]) => !k.startsWith('parcel.'))
                    .map(([k, v]) => (
                      <Fragment key={k}>
                        <dt>{k}</dt>
                        <dd>{v}</dd>
                      </Fragment>
                    ))}
                </dl>
              )}
              <Button icon="download" onClick={() => setExtract({ ids: [one.id] })}>
                Extract this…
              </Button>
            </div>
          ) : (
            <div className="inspector">
              <h2>{m.name}</h2>
              {m.description && <p>{m.description}</p>}
              <dl className="kv">
                <dt>Package ID</dt>
                <dd className="mono" style={{ fontSize: 11 }}>
                  {m.package_id}
                </dd>
                <dt>Created</dt>
                <dd>{formatDateTime(m.created_at)}</dd>
                <dt>Modified</dt>
                <dd>{formatDateTime(m.modified_at)}</dd>
                <dt>Created by</dt>
                <dd>
                  {m.generator.name} {m.generator.version}
                </dd>
                <dt>Content</dt>
                <dd>{formatBytes(entries.reduce((n, e) => n + e.size, 0))}</dd>
              </dl>
              <Button
                icon="folder"
                onClick={() =>
                  api.revealOutput(path).catch((e) => reportError(e, 'Could not show the package'))
                }
              >
                Show package in folder
              </Button>
              <p className="muted small">
                {sel.size > 1
                  ? `${sel.size} items selected.`
                  : 'Select an item to preview it. Nothing in a package is ever run or opened automatically.'}
              </p>
            </div>
          )}
        </aside>
      </div>
      {extract && (
        <ExtractDialog
          count={extract.ids?.length ?? null}
          onClose={() => setExtract(null)}
          onGo={async (p) => {
            if (await doExtract(extract.ids, p)) setExtract(null);
          }}
        />
      )}
    </div>
  );
}

function ExtractDialog({
  count,
  onClose,
  onGo,
}: {
  count: number | null;
  onClose: () => void;
  onGo: (p: ConflictPolicy) => void;
}) {
  const [policy, setPolicy] = useState<ConflictPolicy>('fail');
  return (
    <Dialog
      title={count === null ? 'Extract entire package' : `Extract ${plural(count, 'selected item')}`}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => onGo(policy)}>
            Choose folder and extract
          </Button>
        </>
      }
    >
      <div className="stack">
        <p className="muted">
          Files are verified against their checksums as they are written. Existing files are never
          overwritten.
        </p>
        <fieldset style={{ border: 0, padding: 0, margin: 0 }} className="stack">
          <legend className="label">If a file already exists</legend>
          {(
            [
              ['fail', 'Stop before writing anything'],
              ['rename', 'Keep both (add a number to the new file)'],
              ['skip', 'Skip existing files'],
            ] as [ConflictPolicy, string][]
          ).map(([v, l]) => (
            <label key={v} className="check">
              <input type="radio" name="policy" checked={policy === v} onChange={() => setPolicy(v)} /> {l}
            </label>
          ))}
        </fieldset>
      </div>
    </Dialog>
  );
}

function Checks({
  info,
  report,
  onVerify,
}: {
  info: PackageInfo;
  report: IntegrityReport | null;
  onVerify: () => void;
}) {
  return (
    <div className="content-scroll stack" style={{ paddingTop: 14 }}>
      <section className="stack">
        <div className="row">
          <h2 className="grow">Integrity</h2>
          <Button icon="shield" onClick={onVerify}>
            {report ? 'Verify again' : 'Verify now'}
          </Button>
        </div>
        {!report ? (
          <p className="muted">
            Verification recomputes the SHA-256 checksum of every file and compares it with the manifest.
          </p>
        ) : report.ok ? (
          <Banner tone="ok">
            All {plural(report.checked, 'file')} match their checksums.
            {report.unlisted.length > 0 &&
              ` ${plural(report.unlisted.length, 'unlisted archive member')} found.`}
          </Banner>
        ) : (
          <>
            <Banner tone="error">
              {plural(report.failures.length + report.metadata_failures.length, 'problem')} found. Do not rely
              on the affected files.
            </Banner>
            <table className="table" aria-label="Integrity failures">
              <tbody>
                {report.failures.map((f) => (
                  <tr key={f.id}>
                    <td>{f.path}</td>
                    <td>
                      <Badge tone="danger">{f.status.replace('_', ' ')}</Badge>
                    </td>
                  </tr>
                ))}
                {report.metadata_failures.map((m) => (
                  <tr key={m}>
                    <td colSpan={2}>{m}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
        {report && report.unlisted.length > 0 && (
          <Banner tone="warn">
            Archive members not described by the manifest (ignored): {report.unlisted.join(', ')}
          </Banner>
        )}
      </section>
      <section className="stack">
        <h2>Dependencies &amp; references</h2>
        {info.diagnostics.length === 0 ? (
          <p className="muted">
            No dependency issues.{' '}
            {info.manifest.dependencies.length > 0 &&
              `${plural(info.manifest.dependencies.length, 'dependency', 'dependencies')} declared.`}
          </p>
        ) : (
          info.diagnostics.map((d, i) => (
            <Banner
              key={i}
              tone={d.severity === 'error' ? 'error' : d.severity === 'warning' ? 'warn' : 'info'}
            >
              {d.message}
            </Banner>
          ))
        )}
      </section>
    </div>
  );
}

function CompareView({
  cmp,
  current,
  onPick,
}: {
  cmp: { other: string; result: Comparison } | null;
  current: string;
  onPick: () => void;
}) {
  if (!cmp)
    return (
      <div className="content-scroll">
        <EmptyState
          icon="compare"
          title="Compare with another package"
          action={
            <Button variant="primary" onClick={onPick}>
              Choose an older version…
            </Button>
          }
        >
          See which files were added, removed, changed or moved between two packages. Comparison uses file
          checksums and metadata; it does not analyse what changed inside files.
        </EmptyState>
      </div>
    );
  const c = cmp.result;
  const Sec = ({ title, rows, cls }: { title: string; rows: Comparison['added']; cls: string }) =>
    rows.length === 0 ? null : (
      <section>
        <h3 className={cls}>
          {title} ({rows.length})
        </h3>
        <div className="list">
          {rows.map((r) => (
            <div key={r.path} className="list-row">
              <span className="grow">{r.path}</span>
              <span className="muted small">{r.detail.join(' · ')}</span>
            </div>
          ))}
        </div>
      </section>
    );
  return (
    <div className="content-scroll stack" style={{ paddingTop: 14 }}>
      <div className="row">
        <span className="muted small grow">
          Comparing <strong>{baseName(cmp.other)}</strong> (older) → <strong>{baseName(current)}</strong>{' '}
          (this package). Based on checksums and metadata only.
        </span>
        <Button onClick={onPick}>Change…</Button>
      </div>
      {c.added.length +
        c.removed.length +
        c.changed.length +
        c.moved.length +
        c.metadata_changed.length +
        c.package_differences.length ===
      0 ? (
        <Banner tone="ok">
          The packages are identical ({plural(c.unchanged.length, 'entry', 'entries')} unchanged).
        </Banner>
      ) : (
        <>
          <Sec title="Added" rows={c.added} cls="diff-add" />
          <Sec title="Removed" rows={c.removed} cls="diff-del" />
          <Sec title="Content changed" rows={c.changed} cls="diff-chg" />
          <Sec title="Moved or renamed" rows={c.moved} cls="diff-chg" />
          <Sec title="Metadata changed" rows={c.metadata_changed} cls="diff-chg" />
          {c.package_differences.length > 0 && (
            <section>
              <h3>Package-level differences</h3>
              <ul>
                {c.package_differences.map((d) => (
                  <li key={d}>{d}</li>
                ))}
              </ul>
            </section>
          )}
          <p className="muted small">{plural(c.unchanged.length, 'entry', 'entries')} unchanged.</p>
        </>
      )}
    </div>
  );
}
