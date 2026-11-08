import { useCallback, useEffect, useState } from 'react';
import { useApp } from '../hooks/app';
import { Button, EmptyState, Loading } from '../components/ui';
import { formatDateTime } from '../lib/format';
import type { ActivityRow } from '../lib/types';

export function ActivityPage() {
  const { api, confirm, reportError } = useApp();
  const [rows, setRows] = useState<ActivityRow[] | null>(null);
  const [filter, setFilter] = useState('');
  const load = useCallback(() => {
    api
      .listActivity(null, 300)
      .then(setRows)
      .catch((e) => {
        setRows([]);
        reportError(e);
      });
  }, [api, reportError]);
  useEffect(load, [load]);
  const clear = async () => {
    if (
      !(await confirm({
        title: 'Clear activity history?',
        confirmLabel: 'Clear',
        danger: true,
        message: <p>This removes the history list only. Workspaces and packages are unaffected.</p>,
      }))
    )
      return;
    await api.clearActivity();
    load();
  };
  const shown = rows?.filter(
    (r) =>
      !filter ||
      r.message.toLowerCase().includes(filter.toLowerCase()) ||
      r.kind.includes(filter.toLowerCase()),
  );
  return (
    <div className="page">
      <div className="page-narrow">
        <div className="page-head">
          <div>
            <h1>Activity</h1>
            <p>A local record of what you did in Parcel. It never leaves this computer.</p>
          </div>
          <div className="row">
            <input
              className="input"
              style={{ width: 220 }}
              placeholder="Filter activity"
              aria-label="Filter activity"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            />
            <Button onClick={clear} disabled={!rows?.length}>
              Clear history
            </Button>
          </div>
        </div>
        {!shown ? (
          <Loading />
        ) : shown.length === 0 ? (
          <div className="card">
            <EmptyState icon="activity" title="Nothing here yet">
              {filter
                ? 'No activity matches your filter.'
                : 'Actions such as creating workspaces, exporting and verifying packages will be listed here.'}
            </EmptyState>
          </div>
        ) : (
          <div className="list">
            {shown.map((r) => (
              <div key={r.id} className="list-row">
                <span className="badge">{r.kind.split('.')[0]}</span>
                <div className="grow">{r.message}</div>
                <span className="faint small">{formatDateTime(r.ts)}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
