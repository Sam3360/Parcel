import { useMemo, useState } from 'react';
import { Icon } from '../../components/Icon';
import type { Entry } from '../../lib/types';

export const DRAG_MIME = 'application/x-parcel-ids';

export function Tree({
  entries,
  current,
  onSelect,
  onDropIds,
  rootLabel,
}: {
  entries: Entry[];
  current: string | null;
  onSelect: (id: string | null) => void;
  onDropIds: (ids: string[], target: string | null) => void;
  rootLabel: string;
}) {
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [over, setOver] = useState<string | null | undefined>(undefined);
  const kids = useMemo(() => {
    const m = new Map<string | null, Entry[]>();
    entries
      .filter((e) => e.kind === 'folder')
      .forEach((e) => m.set(e.parent_id, [...(m.get(e.parent_id) ?? []), e]));
    m.forEach((v) => v.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })));
    return m;
  }, [entries]);
  // Keep ancestors of the current folder expanded.
  const forced = useMemo(() => {
    const s = new Set<string>();
    const byId = new Map(entries.map((e) => [e.id, e]));
    let c = current ? byId.get(current) : undefined;
    while (c?.parent_id) {
      s.add(c.parent_id);
      c = byId.get(c.parent_id);
    }
    return s;
  }, [entries, current]);
  const toggle = (id: string) =>
    setOpen((o) => {
      const n = new Set(o);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const dropProps = (target: string | null) => ({
    onDragOver: (e: React.DragEvent) => {
      if (e.dataTransfer.types.includes(DRAG_MIME)) {
        e.preventDefault();
        setOver(target);
      }
    },
    onDragLeave: () => setOver(undefined),
    onDrop: (e: React.DragEvent) => {
      setOver(undefined);
      const raw = e.dataTransfer.getData(DRAG_MIME);
      if (raw) {
        e.preventDefault();
        onDropIds(JSON.parse(raw), target);
      }
    },
  });

  const render = (parent: string | null, depth: number): React.ReactNode =>
    (kids.get(parent) ?? []).map((f) => {
      const has = (kids.get(f.id) ?? []).length > 0;
      const expanded = open.has(f.id) || forced.has(f.id);
      return (
        <div key={f.id} role="none">
          <div role="treeitem" aria-selected={current === f.id} aria-expanded={has ? expanded : undefined}>
            <button
              className={`tree-item ${over === f.id ? 'drop' : ''}`}
              aria-selected={current === f.id}
              style={{ paddingLeft: 6 + depth * 14 }}
              onClick={() => onSelect(f.id)}
              {...dropProps(f.id)}
            >
              <span
                className="tree-chevron"
                role="presentation"
                onClick={(e) => {
                  e.stopPropagation();
                  if (has) toggle(f.id);
                }}
              >
                {has && <Icon name={expanded ? 'down' : 'right'} size={12} />}
              </span>
              <Icon name="folder" className="ficon folder" />
              <span className="ellipsis">{f.name}</span>
            </button>
          </div>
          {has && expanded && <div role="group">{render(f.id, depth + 1)}</div>}
        </div>
      );
    });

  return (
    <div role="tree" aria-label="Workspace structure">
      <div className="panel-title">Structure</div>
      <div role="treeitem" aria-selected={current === null}>
        <button
          className={`tree-item ${over === null ? 'drop' : ''}`}
          aria-selected={current === null}
          onClick={() => onSelect(null)}
          {...dropProps(null)}
        >
          <span className="tree-chevron" />
          <Icon name="layers" />
          <span className="ellipsis">{rootLabel}</span>
        </button>
      </div>
      {render(null, 1)}
    </div>
  );
}
