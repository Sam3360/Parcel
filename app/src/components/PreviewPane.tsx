import { useEffect, useMemo, useState } from 'react';
import { Markdown } from '../lib/markdown';
import { parseCsv } from '../lib/format';
import type { Preview } from '../lib/types';
import { Spinner } from './ui';

/** Renders inert previews. Text is shown as text, images via <img> (never SVG-as-DOM), PDFs via a
 *  sandboxed blob object. No scripts or HTML from file content are ever executed or injected. */
export function PreviewPane({
  load,
  deps,
  onLink,
}: {
  load: () => Promise<Preview>;
  deps: unknown[];
  onLink?: (url: string) => void;
}) {
  const [state, setState] = useState<{ loading: boolean; preview?: Preview; error?: string }>({
    loading: true,
  });
  useEffect(() => {
    let live = true;
    setState({ loading: true });
    load()
      .then((p) => live && setState({ loading: false, preview: p }))
      .catch((e) => live && setState({ loading: false, error: String(e?.message ?? e) }));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  const { loading, preview, error } = state;
  if (loading)
    return (
      <div className="preview" style={{ padding: 16 }}>
        <Spinner label="Loading preview" />
      </div>
    );
  if (error)
    return (
      <div className="preview muted" style={{ padding: 12 }} role="alert">
        Preview failed: {error}
      </div>
    );
  if (!preview) return null;
  return (
    <div className="preview" aria-label="Preview">
      <PreviewBody p={preview} onLink={onLink} />
    </div>
  );
}

function PreviewBody({ p, onLink }: { p: Preview; onLink?: (u: string) => void }) {
  const csv = useMemo(
    () =>
      p.kind === 'csv' && p.text
        ? parseCsv(
            p.text,
            p.text.split('\n')[0]?.includes('\t') && !p.text.split('\n')[0].includes(',') ? '\t' : ',',
          ).slice(0, 200)
        : [],
    [p],
  );
  const pdfUrl = useMemo(() => {
    if (p.kind !== 'pdf' || !p.data_base64) return null;
    const bin = atob(p.data_base64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return URL.createObjectURL(new Blob([bytes], { type: 'application/pdf' }));
  }, [p]);
  useEffect(
    () => () => {
      if (pdfUrl) URL.revokeObjectURL(pdfUrl);
    },
    [pdfUrl],
  );
  switch (p.kind) {
    case 'image':
      return <img alt="File preview" src={`data:${p.mime ?? 'image/png'};base64,${p.data_base64}`} />;
    case 'pdf':
      return pdfUrl ? (
        <object data={pdfUrl} type="application/pdf" aria-label="PDF preview">
          <div className="muted" style={{ padding: 12 }}>
            PDF preview is not available in this viewer.
          </div>
        </object>
      ) : null;
    case 'markdown':
      return (
        <div style={{ padding: '4px 12px', overflow: 'auto', maxHeight: 280 }}>
          <Markdown source={p.text ?? ''} onLink={onLink} />
          {p.truncated && <Trunc />}
        </div>
      );
    case 'csv':
      return (
        <div className="scroll">
          <table>
            <tbody>
              {csv.map((r, i) => (
                <tr key={i}>{r.map((c, j) => (i === 0 ? <th key={j}>{c}</th> : <td key={j}>{c}</td>))}</tr>
              ))}
            </tbody>
          </table>
          {p.truncated && <Trunc />}
        </div>
      );
    case 'json':
    case 'text':
      return (
        <>
          <pre>{p.text}</pre>
          {p.truncated && <Trunc />}
        </>
      );
    default:
      return (
        <div className="muted" style={{ padding: 12 }}>
          {p.reason ?? 'No preview available.'}
        </div>
      );
  }
}
const Trunc = () => (
  <div className="muted small" style={{ padding: '4px 10px' }}>
    Preview truncated — showing the beginning of the file.
  </div>
);
