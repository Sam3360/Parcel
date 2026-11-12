import { useEffect, useState, type MutableRefObject } from 'react';
import { FileIcon } from '../../components/FileIcon';
import { Icon } from '../../components/Icon';
import { Badge } from '../../components/ui';
import { extOf, formatBytes, formatDate } from '../../lib/format';
import type { Entry, SourceState } from '../../lib/types';
import { SpacerRow, useVirtualRows } from '../../lib/virtual';
import { DRAG_MIME } from './Tree';

export interface ViewApi {
  /** Bring an item into view (and into the DOM, if the view only renders a window of items). */
  scrollToId: (id: string) => void;
}

/** Grid view renders this many tiles at a time; a button reveals more. */
export const GRID_CHUNK = 400;

export interface ViewProps {
  items: Entry[];
  selected: Set<string>;
  paths: Map<string, string> | null;
  status: Map<string, SourceState>;
  sort: { key: string; desc: boolean };
  onSort: (key: string) => void;
  onClick: (e: React.MouseEvent, id: string) => void;
  onOpen: (e: Entry) => void;
  onContext: (e: React.MouseEvent, id: string | null) => void;
  apiRef?: MutableRefObject<ViewApi | null>;
}

export function StatusBadge({ s }: { s?: SourceState }) {
  if (!s || s.state === 'unchanged') return null;
  if (s.state === 'missing')
    return (
      <Badge tone="danger" title="The source file could not be found">
        Missing
      </Badge>
    );
  if (s.state === 'changed')
    return (
      <Badge tone="warn" title={s.reason}>
        Changed
      </Badge>
    );
  return (
    <Badge tone="danger" title={s.reason}>
      Unreadable
    </Badge>
  );
}

function dragStart(e: React.DragEvent, id: string, selected: Set<string>) {
  const ids = selected.has(id) ? [...selected] : [id];
  e.dataTransfer.setData(DRAG_MIME, JSON.stringify(ids));
  e.dataTransfer.effectAllowed = 'move';
}

const kindLabel: Record<string, string> = { file: 'File', folder: 'Folder', note: 'Note', link: 'Link' };

export function ListView(p: ViewProps) {
  const v = useVirtualRows(p.items.length);
  const { items, apiRef } = p;
  useEffect(() => {
    if (!apiRef) return;
    apiRef.current = {
      scrollToId: (id) => {
        const i = items.findIndex((x) => x.id === id);
        if (i < 0) return;
        if (v.enabled) v.scrollToIndex(i);
        else document.querySelector(`[data-entry-id="${id}"]`)?.scrollIntoView?.({ block: 'nearest' });
      },
    };
  });
  const th = (key: string, label: string) => (
    <th aria-sort={p.sort.key === key ? (p.sort.desc ? 'descending' : 'ascending') : 'none'}>
      <button onClick={() => p.onSort(key)}>
        {label}
        {p.sort.key === key && <Icon name={p.sort.desc ? 'down' : 'right'} size={10} className="" />}
      </button>
    </th>
  );
  return (
    <table className="table virtual" aria-label="Workspace contents" aria-rowcount={p.items.length + 1}>
      <thead>
        <tr>
          {th('name', 'Name')}
          {p.paths && <th>Location</th>}
          {th('kind', 'Type')}
          {th('size', 'Size')}
          {th('modified', 'Modified')}
          <th>Status</th>
        </tr>
      </thead>
      <tbody ref={v.tbodyRef}>
        <SpacerRow height={v.topPad} cols={p.paths ? 6 : 5} />
        {p.items.slice(v.start, v.end).map((e, k) => (
          <tr
            key={e.id}
            data-vrow
            aria-rowindex={v.start + k + 2}
            aria-selected={p.selected.has(e.id)}
            data-entry-id={e.id}
            draggable
            onDragStart={(ev) => dragStart(ev, e.id, p.selected)}
            onClick={(ev) => p.onClick(ev, e.id)}
            onDoubleClick={() => p.onOpen(e)}
            onContextMenu={(ev) => {
              ev.preventDefault();
              ev.stopPropagation();
              p.onContext(ev, e.id);
            }}
          >
            <td>
              <div className="row">
                <FileIcon kind={e.kind} name={e.name} mime={e.mime} />
                <span className="ellipsis" style={{ maxWidth: 360 }}>
                  {e.name}
                </span>
                {e.tags.slice(0, 2).map((t) => (
                  <Badge key={t}>{t}</Badge>
                ))}
              </div>
            </td>
            {p.paths && (
              <td className="muted small ellipsis" style={{ maxWidth: 220 }}>
                {(p.paths.get(e.id) ?? '').split('/').slice(0, -1).join('/') || '/'}
              </td>
            )}
            <td className="muted">
              {e.kind === 'file' ? extOf(e.name).toUpperCase() || 'File' : kindLabel[e.kind]}
            </td>
            <td className="muted">{e.kind === 'folder' || e.kind === 'link' ? '' : formatBytes(e.size)}</td>
            <td className="muted">{formatDate(e.modified_at)}</td>
            <td>
              <StatusBadge s={p.status.get(e.id)} />
            </td>
          </tr>
        ))}
        <SpacerRow height={v.bottomPad} cols={p.paths ? 6 : 5} />
      </tbody>
    </table>
  );
}

export function GridView(p: ViewProps) {
  const [limit, setLimit] = useState(GRID_CHUNK);
  const { items, apiRef } = p;
  const shown = Math.min(limit, items.length);
  const [pending, setPending] = useState<string | null>(null);
  useEffect(() => {
    if (!apiRef) return;
    apiRef.current = {
      scrollToId: (id) => {
        const i = items.findIndex((x) => x.id === id);
        if (i < 0) return;
        if (i >= limit) {
          setLimit(Math.ceil((i + 1) / GRID_CHUNK) * GRID_CHUNK);
          setPending(id); // scrolled once the tile exists
        } else document.querySelector(`[data-entry-id="${id}"]`)?.scrollIntoView?.({ block: 'nearest' });
      },
    };
  });
  useEffect(() => {
    if (pending) {
      document.querySelector(`[data-entry-id="${pending}"]`)?.scrollIntoView?.({ block: 'nearest' });
      setPending(null);
    }
  }, [pending, shown]);
  return (
    <>
      <div className="tiles" role="listbox" aria-label="Workspace contents" aria-multiselectable>
        {items.slice(0, shown).map((e) => (
          <div
            key={e.id}
            role="option"
            className="tile"
            aria-selected={p.selected.has(e.id)}
            data-entry-id={e.id}
            draggable
            onDragStart={(ev) => dragStart(ev, e.id, p.selected)}
            onClick={(ev) => p.onClick(ev, e.id)}
            onDoubleClick={() => p.onOpen(e)}
            onContextMenu={(ev) => {
              ev.preventDefault();
              ev.stopPropagation();
              p.onContext(ev, e.id);
            }}
          >
            <span className="status">
              <StatusBadge s={p.status.get(e.id)} />
            </span>
            <FileIcon kind={e.kind} name={e.name} mime={e.mime} size={34} />
            <span className="name" title={e.name}>
              {e.name}
            </span>
            <span className="faint small">
              {e.kind === 'file' || e.kind === 'note' ? formatBytes(e.size) : kindLabel[e.kind]}
            </span>
          </div>
        ))}
      </div>
      {shown < items.length && (
        <div className="row" style={{ justifyContent: 'center', padding: 12 }}>
          <button className="btn" onClick={() => setLimit(shown + GRID_CHUNK)}>
            Show {Math.min(GRID_CHUNK, items.length - shown)} more ({items.length - shown} not shown)
          </button>
        </div>
      )}
    </>
  );
}
