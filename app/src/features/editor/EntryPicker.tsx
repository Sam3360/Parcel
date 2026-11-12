import { useMemo, useState } from 'react';
import { Dialog } from '../../components/Dialog';
import { Button } from '../../components/ui';
import type { Entry } from '../../lib/types';

/** At most this many matches are rendered, however large the workspace is. */
export const PICKER_LIMIT = 100;

/**
 * A filterable item chooser. A native <select> would mount one <option> per workspace item on every selection
 * change, which made arrow-key navigation slow in large workspaces; this renders nothing until opened and then
 * at most PICKER_LIMIT matches.
 */
export function EntryPicker({
  entries,
  paths,
  value,
  onChange,
  label,
  emptyLabel = 'Choose…',
  clearLabel,
  grow,
}: {
  entries: Entry[];
  paths: Map<string, string>;
  value: string | null;
  onChange: (id: string | null) => void;
  label: string;
  emptyLabel?: string;
  /** When set, the dialog offers a first row that clears the value (e.g. "Whole workspace"). */
  clearLabel?: string;
  grow?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState('');
  const current = value ? (paths.get(value) ?? value) : (clearLabel ?? emptyLabel);
  const matches = useMemo(() => {
    if (!open) return { rows: [] as Entry[], total: 0 };
    const terms = q.toLowerCase().split(/\s+/).filter(Boolean);
    const all = entries.filter((e) => {
      const p = (paths.get(e.id) ?? e.name).toLowerCase();
      return terms.every((t) => p.includes(t));
    });
    return { rows: all.slice(0, PICKER_LIMIT), total: all.length };
  }, [open, q, entries, paths]);
  const close = () => {
    setOpen(false);
    setQ('');
  };
  const pick = (id: string | null) => {
    close();
    onChange(id);
  };
  return (
    <>
      <button
        type="button"
        className={`select ${grow ? 'grow' : ''}`}
        style={{ textAlign: 'left' }}
        aria-label={label}
        aria-haspopup="dialog"
        onClick={() => setOpen(true)}
      >
        <span className="ellipsis">{current}</span>
      </button>
      {open && (
        <Dialog title={label} onClose={close} footer={<Button onClick={close}>Cancel</Button>}>
          <div className="stack">
            <input
              className="input"
              aria-label="Filter items"
              placeholder="Type to filter by name or folder"
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
            <div
              role="listbox"
              aria-label="Items"
              className="list"
              style={{ maxHeight: 320, overflow: 'auto' }}
            >
              {clearLabel && (
                <button
                  role="option"
                  aria-selected={value === null}
                  className="list-row"
                  onClick={() => pick(null)}
                >
                  {clearLabel}
                </button>
              )}
              {matches.rows.map((e) => (
                <button
                  key={e.id}
                  role="option"
                  aria-selected={value === e.id}
                  className="list-row"
                  onClick={() => pick(e.id)}
                >
                  <span className="ellipsis">{paths.get(e.id) ?? e.name}</span>
                </button>
              ))}
              {matches.total === 0 && <div className="list-row muted">No matching items.</div>}
            </div>
            {matches.total > PICKER_LIMIT && (
              <p className="muted small">
                Showing the first {PICKER_LIMIT} of {matches.total} matches. Keep typing to narrow the list.
              </p>
            )}
          </div>
        </Dialog>
      )}
    </>
  );
}
