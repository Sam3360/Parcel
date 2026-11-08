import { useState } from 'react';
import { Dialog } from '../components/Dialog';
import { Banner, Button, ErrorText } from '../components/ui';
import { describeError, errorCode } from '../lib/api';
import { baseName, formatBytes, plural } from '../lib/format';
import { useApp } from '../hooks/app';
import type { ZipImportResult } from '../lib/types';

/** Converts a ZIP archive into a new .parcel file (the ZIP itself is never modified). */
export function ZipImport({ onClose }: { onClose: () => void }) {
  const { api, runJob, navigate } = useApp();
  const [zip, setZip] = useState<string | null>(null);
  const [dest, setDest] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ZipImportResult | null>(null);

  const pickZip = async () => {
    const z = await api.pickZip();
    if (z) {
      setZip(z);
      setDest(null);
      setError(null);
    }
  };
  const pickDest = async () => {
    if (!zip) return;
    const d = await api.pickSaveParcel(baseName(zip).replace(/\.zip$/i, '') + '.parcel');
    if (d) setDest(d);
  };
  const run = async () => {
    if (!zip || !dest) return;
    setError(null);
    try {
      // The save dialog already asked the user to confirm replacing an existing file.
      const name = baseName(zip).replace(/\.zip$/i, '');
      setResult(
        await runJob('Converting ZIP to a Parcel package', (id) => api.convertZip(zip, dest, name, true, id)),
      );
    } catch (e) {
      setError(errorCode(e) === 'cancelled' ? 'Cancelled. Nothing was written.' : describeError(e));
    }
  };
  return (
    <Dialog
      title="Import a ZIP archive"
      onClose={onClose}
      footer={
        result ? (
          <>
            <Button onClick={onClose}>Close</Button>
            <Button
              variant="primary"
              onClick={() => {
                onClose();
                navigate({ page: 'open', path: result.path });
              }}
            >
              Open package
            </Button>
          </>
        ) : (
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button variant="primary" disabled={!zip || !dest} onClick={run}>
              Convert
            </Button>
          </>
        )
      }
    >
      <div className="stack">
        <p className="muted">
          Creates a new .parcel package from a ZIP file. The archive is checked for unsafe paths, links and
          oversized content first, and is never changed.
        </p>
        <div className="row">
          <Button onClick={pickZip}>{zip ? 'Change ZIP…' : 'Choose ZIP…'}</Button>
          <span className="ellipsis muted">{zip ?? 'No file chosen'}</span>
        </div>
        <div className="row">
          <Button onClick={pickDest} disabled={!zip}>
            {dest ? 'Change destination…' : 'Save package as…'}
          </Button>
          <span className="ellipsis muted">{dest ?? 'No destination chosen'}</span>
        </div>
        <ErrorText error={error} />
        {result && (
          <Banner tone="ok">
            Created {baseName(result.path)}: {plural(result.entries, 'entry', 'entries')},{' '}
            {formatBytes(result.size)}.
            {result.warnings.length > 0 && (
              <ul style={{ margin: '6px 0 0', paddingLeft: 18 }}>
                {result.warnings.map((w, i) => (
                  <li key={i}>{w}</li>
                ))}
              </ul>
            )}
          </Banner>
        )}
      </div>
    </Dialog>
  );
}
