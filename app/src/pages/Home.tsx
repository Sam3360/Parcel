import { useCallback, useEffect, useState } from 'react';
import { useApp } from '../hooks/app';
import { Button, EmptyState, Loading } from '../components/ui';
import { Icon } from '../components/Icon';
import { NewWorkspaceDialog } from '../features/NewWorkspaceDialog';
import { ZipImport } from '../features/ZipImport';
import { formatBytes, plural, relativeTime } from '../lib/format';
import type { RecentPackage, WorkspaceSummary } from '../lib/types';

export function HomePage() {
  const { api, navigate, reportError } = useApp();
  const [recent, setRecent] = useState<WorkspaceSummary[] | null>(null);
  const [pkgs, setPkgs] = useState<RecentPackage[] | null>(null);
  const [name, setName] = useState('');
  const [creating, setCreating] = useState(false);
  const [zip, setZip] = useState(false);

  const load = useCallback(() => {
    api
      .recentWorkspaces(6)
      .then(setRecent)
      .catch((e) => {
        setRecent([]);
        reportError(e);
      });
    api
      .listRecentPackages(6)
      .then(setPkgs)
      .catch(() => setPkgs([]));
  }, [api, reportError]);
  useEffect(load, [load]);

  const openPackage = async () => {
    const p = await api.pickPackage();
    if (p) navigate({ page: 'open', path: p });
  };

  return (
    <div className="page">
      <div className="page-narrow">
        <div className="hero">
          <h1>Everything that belongs together, packaged together.</h1>
          <p className="muted" style={{ marginBottom: 16 }}>
            Collect files, notes and links from anywhere, organise them, and pack them into one portable
            .parcel file.
          </p>
          <form
            className="quick"
            onSubmit={(e) => {
              e.preventDefault();
              if (name.trim()) setCreating(true);
            }}
          >
            <input
              className="input"
              placeholder="Name a new workspace…"
              aria-label="New workspace name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <Button variant="primary" icon="plus" type="submit" disabled={!name.trim()}>
              Create
            </Button>
          </form>
          <div className="row" style={{ marginTop: 12 }}>
            <Button icon="package" onClick={openPackage}>
              Open package…
            </Button>
            <Button icon="download" onClick={() => setZip(true)}>
              Import ZIP…
            </Button>
            <Button variant="ghost" onClick={() => setCreating(true)}>
              From a template…
            </Button>
          </div>
        </div>

        <section className="section" aria-labelledby="recent-ws">
          <header>
            <h2 id="recent-ws">Recent workspaces</h2>
            {recent && recent.length > 0 && (
              <Button variant="ghost" onClick={() => navigate({ page: 'workspaces' })}>
                View all
              </Button>
            )}
          </header>
          {recent === null ? (
            <Loading />
          ) : recent.length === 0 ? (
            <div className="card">
              <EmptyState icon="layers" title="No workspaces yet">
                A workspace is where you gather and arrange material before packaging it. Name one above to
                begin: you can drag in files and folders, write notes, and save links.
              </EmptyState>
              <div
                className="row"
                style={{ justifyContent: 'center', gap: 28, paddingBottom: 24 }}
                aria-label="How Parcel works"
              >
                {[
                  ['plus', '1. Collect', 'Files stay where they are'],
                  ['folder', '2. Organise', 'Folders, notes, links'],
                  ['package', '3. Package', 'One portable .parcel'],
                ].map(([i, t, s]) => (
                  <div key={t} className="stack" style={{ alignItems: 'center', gap: 2 }}>
                    <Icon name={i as 'plus'} />
                    <strong>{t}</strong>
                    <span className="muted small">{s}</span>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="grid-cards">
              {recent.map((w) => (
                <button
                  key={w.id}
                  className="ws-card"
                  onClick={() => navigate({ page: 'workspaces', workspaceId: w.id })}
                >
                  <span className="title ellipsis">{w.name}</span>
                  <span className="muted small">
                    {plural(w.entry_count, 'item')} · {formatBytes(w.total_size)}
                  </span>
                  <span className="faint small">Edited {relativeTime(w.modified_at)}</span>
                </button>
              ))}
            </div>
          )}
        </section>

        <section className="section" aria-labelledby="recent-pk">
          <header>
            <h2 id="recent-pk">Recent packages</h2>
          </header>
          {pkgs === null ? (
            <Loading />
          ) : pkgs.length === 0 ? (
            <div className="card">
              <EmptyState icon="package" title="No packages opened yet">
                Open any .parcel file to browse, verify and extract it — no workspace needed.
              </EmptyState>
            </div>
          ) : (
            <div className="list">
              {pkgs.map((p) => (
                <button
                  key={p.path}
                  className="list-row"
                  onClick={() => navigate({ page: 'open', path: p.path })}
                >
                  <Icon name="package" />
                  <div className="grow">
                    <div className="ellipsis">{p.name}</div>
                    <div className="muted small ellipsis">{p.path}</div>
                  </div>
                  <span className="muted small">{formatBytes(p.size)}</span>
                  <span className="faint small">{relativeTime(p.opened_at)}</span>
                </button>
              ))}
            </div>
          )}
        </section>
        {creating && (
          <NewWorkspaceDialog
            initialName={name}
            onClose={() => setCreating(false)}
            onCreated={(w) => {
              setCreating(false);
              setName('');
              navigate({ page: 'workspaces', workspaceId: w.id });
            }}
          />
        )}
        {zip && <ZipImport onClose={() => setZip(false)} />}
      </div>
    </div>
  );
}
