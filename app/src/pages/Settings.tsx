import { useEffect, useState } from 'react';
import { useApp } from '../hooks/app';
import { Button, Field } from '../components/ui';
import { SHORTCUTS } from '../lib/shortcuts';
import { describeError } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { StorageReport } from '../lib/types';

const DEFAULT_LIMITS = {
  max_entries: 100000,
  max_total_uncompressed: 64 * 1024 ** 3,
  max_entry_uncompressed: 32 * 1024 ** 3,
  max_compression_ratio: 1000,
  max_manifest_bytes: 64 * 1024 ** 2,
  max_path_len: 1024,
  max_path_depth: 64,
};
type Limits = typeof DEFAULT_LIMITS;
const toText = (l: Limits) =>
  Object.fromEntries(Object.entries(l).map(([k, v]) => [k, String(v)])) as Record<keyof Limits, string>;

export function SettingsPage() {
  const { api, prefs, setPref, reportError, toast, showShortcuts, confirm } = useApp();
  const [storage, setStorage] = useState<StorageReport | 'error' | null>(null);
  const [cleaning, setCleaning] = useState(false);
  const loadStorage = () => {
    setStorage(null);
    api
      .storageReport()
      .then(setStorage)
      .catch(() => setStorage('error'));
  };
  useEffect(loadStorage, [api]);
  const cleanStorage = async () => {
    if (!storage || storage === 'error') return;
    const ok = await confirm({
      title: 'Remove unused stored files?',
      confirmLabel: 'Remove unused files',
      danger: true,
      message: (
        <>
          <p>
            This deletes {plural(storage.unused_files, 'file')} ({formatBytes(storage.unused_bytes)})
            {storage.orphan_folders > 0 ? ` and ${plural(storage.orphan_folders, 'leftover folder')}` : ''}{' '}
            from Parcel's own storage. Nothing in a workspace refers to them.
          </p>
          <p className="muted">
            Your original files, your exported packages, and everything your workspaces use are not touched.
            Parcel checks again just before deleting.
          </p>
        </>
      ),
    });
    if (!ok) return;
    setCleaning(true);
    try {
      const r = await api.removeUnusedStorage();
      toast(`Freed ${formatBytes(r.bytes_freed)}`, 'success');
    } catch (e) {
      reportError(e, 'Could not clean up storage');
    } finally {
      setCleaning(false);
      loadStorage();
    }
  };
  const [info, setInfo] = useState<{
    version: string;
    data_dir: string;
    schema_version: number;
    format_version: string;
  } | null>(null);
  const [limits, setLimits] = useState<Record<keyof Limits, string>>(toText(DEFAULT_LIMITS));
  useEffect(() => {
    api
      .appInfo()
      .then(setInfo)
      .catch(() => {});
  }, [api]);
  useEffect(() => {
    try {
      if (prefs.limits) setLimits(toText({ ...DEFAULT_LIMITS, ...JSON.parse(prefs.limits) }));
    } catch {
      /* keep defaults */
    }
  }, [prefs.limits]);

  const save = async (k: string, v: string) => {
    try {
      await setPref(k, v);
    } catch (e) {
      reportError(e, 'Could not save setting');
    }
  };
  const saveLimits = async () => {
    const parsed: Record<string, number> = {};
    for (const [k, v] of Object.entries(limits)) {
      const n = Number(v);
      if (!Number.isInteger(n) || n < 1) {
        toast(`“${k.replace(/_/g, ' ')}” must be a whole number of at least 1`, 'error');
        return;
      }
      parsed[k] = n;
    }
    try {
      await setPref('limits', JSON.stringify(parsed));
      toast('Security limits saved', 'success');
    } catch (e) {
      toast(describeError(e), 'error');
    }
  };
  const num = (k: keyof Limits, label: string, hint?: string) => (
    <Field label={label} hint={hint} htmlFor={`lim-${k}`}>
      <input
        id={`lim-${k}`}
        className="input"
        type="text"
        value={limits[k]}
        inputMode="numeric"
        onChange={(e) => setLimits({ ...limits, [k]: e.target.value })}
      />
    </Field>
  );
  return (
    <div className="page">
      <div className="page-narrow" style={{ maxWidth: 760 }}>
        <div className="page-head">
          <div>
            <h1>Settings</h1>
            <p>Preferences are stored locally on this computer.</p>
          </div>
        </div>
        <section className="card card-pad stack" aria-labelledby="s-app">
          <h2 id="s-app">Appearance</h2>
          <Field label="Theme" htmlFor="set-theme">
            <select
              id="set-theme"
              className="select"
              value={prefs.theme ?? 'system'}
              onChange={(e) => save('theme', e.target.value)}
            >
              <option value="system">Match system</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </Field>
          <Field label="Density" htmlFor="set-density">
            <select
              id="set-density"
              className="select"
              value={prefs.density ?? 'comfortable'}
              onChange={(e) => save('density', e.target.value)}
            >
              <option value="comfortable">Comfortable</option>
              <option value="compact">Compact</option>
            </select>
          </Field>
          <Field label="Default view for workspace contents" htmlFor="set-view">
            <select
              id="set-view"
              className="select"
              value={prefs.view ?? 'list'}
              onChange={(e) => save('view', e.target.value)}
            >
              <option value="list">List</option>
              <option value="grid">Grid</option>
            </select>
          </Field>
          <label className="check">
            <input
              type="checkbox"
              checked={prefs.reduce_motion === 'true'}
              onChange={(e) => save('reduce_motion', String(e.target.checked))}
            />{' '}
            Reduce motion
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={prefs.confirm_remove !== 'false'}
              onChange={(e) => save('confirm_remove', String(e.target.checked))}
            />{' '}
            Ask before removing items from a workspace
          </label>
        </section>

        <section className="card card-pad stack section" aria-labelledby="s-sec">
          <h2 id="s-sec">Security limits for opening packages</h2>
          <p className="muted">
            Packages from other people are untrusted. These limits stop oversized or malicious archives (for
            example ZIP bombs) before they can use up your disk or memory. Defaults are generous; lower them
            for stricter protection.
          </p>
          <div className="grid-cards" style={{ gridTemplateColumns: 'repeat(2, 1fr)' }}>
            {num('max_entries', 'Maximum items per package')}
            {num(
              'max_compression_ratio',
              'Maximum compression ratio',
              'Larger ratios are treated as ZIP bombs.',
            )}
            {num('max_total_uncompressed', 'Maximum total size (bytes)')}
            {num('max_entry_uncompressed', 'Maximum size of one file (bytes)')}
          </div>
          <div className="row">
            <Button variant="primary" onClick={saveLimits}>
              Save limits
            </Button>
            <Button variant="ghost" onClick={() => setLimits(toText(DEFAULT_LIMITS))}>
              Reset to defaults
            </Button>
          </div>
        </section>

        <section className="card card-pad stack section" aria-labelledby="s-store">
          <h2 id="s-store">Storage</h2>
          <p className="muted">
            Items imported from a package are stored by Parcel so the workspace is self-contained. Files you
            collect from your computer stay where they are and are never stored or deleted by Parcel.
          </p>
          {storage === null ? (
            <p className="muted">Checking…</p>
          ) : storage === 'error' ? (
            <p className="muted">Storage information is not available.</p>
          ) : (
            <>
              <dl className="kv" aria-label="Storage summary">
                <dt>Imported files</dt>
                <dd>
                  {plural(storage.files, 'file')} ({formatBytes(storage.bytes)})
                </dd>
                <dt>Not used by anything</dt>
                <dd>
                  {plural(storage.unused_files, 'file')} ({formatBytes(storage.unused_bytes)})
                  {storage.orphan_folders > 0 && `, ${plural(storage.orphan_folders, 'leftover folder')}`}
                </dd>
              </dl>
              {storage.unused_examples.length > 0 && (
                <ul
                  className="mono small"
                  aria-label="Examples of unused files"
                  style={{ margin: 0, paddingLeft: 18 }}
                >
                  {storage.unused_examples.map((x) => (
                    <li key={x}>{x}</li>
                  ))}
                </ul>
              )}
              <div className="row">
                <Button
                  className="danger"
                  disabled={cleaning || (storage.unused_files === 0 && storage.orphan_folders === 0)}
                  onClick={cleanStorage}
                >
                  Remove unused files…
                </Button>
                <Button variant="ghost" onClick={loadStorage}>
                  Refresh
                </Button>
              </div>
            </>
          )}
        </section>

        <section className="card card-pad stack section" aria-labelledby="s-priv">
          <h2 id="s-priv">Privacy</h2>
          <p>
            Parcel works entirely offline. It has no account, no cloud service, no telemetry, no analytics and
            makes no background network requests. Links are only opened in your browser when you click them.
          </p>
        </section>

        <section className="card card-pad stack section" aria-labelledby="s-about">
          <h2 id="s-about">About</h2>
          <dl className="kv">
            <dt>Version</dt>
            <dd>{info?.version ?? '—'}</dd>
            <dt>Package format</dt>
            <dd>{info?.format_version ?? '—'}</dd>
            <dt>Database schema</dt>
            <dd>{info?.schema_version ?? '—'}</dd>
            <dt>Data folder</dt>
            <dd className="mono">{info?.data_dir ?? '—'}</dd>
          </dl>
          <div>
            <Button icon="keyboard" onClick={showShortcuts}>
              Keyboard shortcuts ({SHORTCUTS.length})
            </Button>
          </div>
        </section>
      </div>
    </div>
  );
}
