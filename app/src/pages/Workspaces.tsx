import { useCallback, useEffect, useState } from 'react';
import { useApp } from '../hooks/app';
import { Button, EmptyState, IconButton, Loading } from '../components/ui';
import { ContextMenu, type MenuItem } from '../components/ContextMenu';
import { NewWorkspaceDialog } from '../features/NewWorkspaceDialog';
import { WorkspaceEditor } from '../features/WorkspaceEditor';
import { formatBytes, plural, relativeTime } from '../lib/format';
import type { WorkspaceSummary } from '../lib/types';

export function WorkspacesPage() {
  const { route, navigate } = useApp();
  if (route.page === 'workspaces' && route.workspaceId) {
    return (
      <WorkspaceEditor
        key={route.workspaceId}
        id={route.workspaceId}
        onBack={() => navigate({ page: 'workspaces' })}
      />
    );
  }
  return <WorkspaceList />;
}

function WorkspaceList() {
  const { api, navigate, prompt, confirm, toast, reportError } = useApp();
  const [items, setItems] = useState<WorkspaceSummary[] | null>(null);
  const [trash, setTrash] = useState(false);
  const [creating, setCreating] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; w: WorkspaceSummary } | null>(null);

  const load = useCallback(() => {
    setItems(null);
    api
      .listWorkspaces(trash)
      .then(setItems)
      .catch((e) => {
        setItems([]);
        reportError(e);
      });
  }, [api, trash, reportError]);
  useEffect(load, [load]);

  const rename = async (w: WorkspaceSummary) => {
    const n = await prompt({
      title: 'Rename workspace',
      label: 'Name',
      initial: w.name,
      confirmLabel: 'Rename',
    });
    if (!n) return;
    try {
      await api.updateWorkspace(w.id, { name: n });
      load();
    } catch (e) {
      reportError(e, 'Could not rename');
    }
  };
  const duplicate = async (w: WorkspaceSummary) => {
    const n = await prompt({
      title: 'Duplicate workspace',
      label: 'Name for the copy',
      initial: `${w.name} copy`,
      confirmLabel: 'Duplicate',
    });
    if (!n) return;
    try {
      await api.duplicateWorkspace(w.id, n);
      toast(`Created “${n}”`, 'success');
      load();
    } catch (e) {
      reportError(e, 'Could not duplicate');
    }
  };
  const moveToTrash = async (w: WorkspaceSummary) => {
    const ok = await confirm({
      title: 'Move workspace to trash?',
      confirmLabel: 'Move to trash',
      danger: true,
      message: (
        <p>
          “{w.name}” will be moved to the trash. Your original files are not touched and you can restore the
          workspace later.
        </p>
      ),
    });
    if (!ok) return;
    try {
      await api.trashWorkspace(w.id);
      toast('Moved to trash');
      load();
    } catch (e) {
      reportError(e);
    }
  };
  const purge = async (w: WorkspaceSummary) => {
    const ok = await confirm({
      title: 'Delete permanently?',
      confirmLabel: 'Delete permanently',
      danger: true,
      message: (
        <p>
          This permanently removes “{w.name}” and its notes, references and organisation from Parcel. Original
          files and exported packages are not deleted. This cannot be undone.
        </p>
      ),
    });
    if (!ok) return;
    try {
      await api.purgeWorkspace(w.id);
      load();
    } catch (e) {
      reportError(e);
    }
  };
  const restore = async (w: WorkspaceSummary) => {
    try {
      await api.restoreWorkspace(w.id);
      toast('Restored', 'success');
      load();
    } catch (e) {
      reportError(e);
    }
  };

  const menuItems = (w: WorkspaceSummary): MenuItem[] =>
    trash
      ? [
          { label: 'Restore', icon: 'refresh', onSelect: () => restore(w) },
          { separator: true },
          { label: 'Delete permanently…', icon: 'trash', danger: true, onSelect: () => purge(w) },
        ]
      : [
          {
            label: 'Open',
            icon: 'folder',
            onSelect: () => navigate({ page: 'workspaces', workspaceId: w.id }),
          },
          { label: 'Rename…', icon: 'edit', onSelect: () => rename(w) },
          { label: 'Duplicate…', icon: 'copy', onSelect: () => duplicate(w) },
          { separator: true },
          { label: 'Move to trash…', icon: 'trash', danger: true, onSelect: () => moveToTrash(w) },
        ];

  return (
    <div className="page">
      <div className="page-narrow">
        <div className="page-head">
          <div>
            <h1>{trash ? 'Trash' : 'Workspaces'}</h1>
            <p>
              {trash
                ? 'Restore a workspace or remove it for good.'
                : 'Everything you are collecting, in one place.'}
            </p>
          </div>
          <div className="row">
            <Button aria-pressed={trash} icon="trash" onClick={() => setTrash((t) => !t)}>
              {trash ? 'Back to workspaces' : 'Trash'}
            </Button>
            {!trash && (
              <Button variant="primary" icon="plus" onClick={() => setCreating(true)}>
                New workspace
              </Button>
            )}
          </div>
        </div>
        {items === null ? (
          <Loading />
        ) : items.length === 0 ? (
          <div className="card">
            <EmptyState
              icon={trash ? 'trash' : 'layers'}
              title={trash ? 'The trash is empty' : 'No workspaces yet'}
              action={
                !trash ? (
                  <Button variant="primary" icon="plus" onClick={() => setCreating(true)}>
                    New workspace
                  </Button>
                ) : undefined
              }
            >
              {trash
                ? 'Workspaces you remove will appear here.'
                : 'Create your first workspace to start collecting files, notes and links.'}
            </EmptyState>
          </div>
        ) : (
          <div className="list" role="list">
            {items.map((w) => (
              <div
                key={w.id}
                role="listitem"
                className="list-row"
                onContextMenu={(e) => {
                  e.preventDefault();
                  setMenu({ x: e.clientX, y: e.clientY, w });
                }}
              >
                <button
                  className="grow"
                  style={{ all: 'unset', cursor: trash ? 'default' : 'pointer', flex: 1, minWidth: 0 }}
                  onClick={() => !trash && navigate({ page: 'workspaces', workspaceId: w.id })}
                  aria-label={trash ? undefined : `Open ${w.name}`}
                >
                  <div style={{ fontWeight: 600 }} className="ellipsis">
                    {w.name}
                  </div>
                  <div className="muted small ellipsis">
                    {w.description || `${plural(w.entry_count, 'item')} · ${formatBytes(w.total_size)}`}
                  </div>
                </button>
                <span className="muted small">{plural(w.entry_count, 'item')}</span>
                <span className="muted small">{w.revision > 0 ? `rev ${w.revision}` : 'not exported'}</span>
                <span className="faint small" style={{ width: 90, textAlign: 'right' }}>
                  {relativeTime(w.modified_at)}
                </span>
                <IconButton
                  icon="more"
                  label={`Actions for ${w.name}`}
                  onClick={(e) => {
                    const r = e.currentTarget.getBoundingClientRect();
                    setMenu({ x: r.left, y: r.bottom, w });
                  }}
                />
              </div>
            ))}
          </div>
        )}
        {menu && (
          <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.w)} onClose={() => setMenu(null)} />
        )}
        {creating && (
          <NewWorkspaceDialog
            onClose={() => setCreating(false)}
            onCreated={(w) => {
              setCreating(false);
              navigate({ page: 'workspaces', workspaceId: w.id });
            }}
          />
        )}
      </div>
    </div>
  );
}
