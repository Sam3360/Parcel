import { useCallback, useLayoutEffect, useRef, useState, type RefObject } from 'react';

/** Below this many rows nothing is virtualised: small lists behave exactly as before. */
export const VIRTUALIZE_MIN_ROWS = 80;
const OVERSCAN = 12;

export interface VirtualRows {
  tbodyRef: RefObject<HTMLTableSectionElement>;
  start: number;
  end: number;
  topPad: number;
  bottomPad: number;
  /** True when only a window of rows is rendered. */
  enabled: boolean;
  scrollToIndex: (index: number) => void;
}

function cssRowHeight(): number {
  const raw = getComputedStyle(document.documentElement).getPropertyValue('--row-h');
  const n = parseFloat(raw);
  return Number.isFinite(n) && n > 0 ? n : 32;
}

/**
 * Window rows of a fixed-height table inside its `.content-scroll` container. Sorting, filtering and selection
 * live outside: this hook only decides which slice of the already-ordered rows is in the DOM, and offers
 * `scrollToIndex` so keyboard navigation can reach rows that are not currently rendered.
 * Without layout information (no scroll container height, e.g. jsdom) every row renders.
 */
export function useVirtualRows(count: number, minRows = VIRTUALIZE_MIN_ROWS): VirtualRows {
  const tbodyRef = useRef<HTMLTableSectionElement>(null);
  const [rowH, setRowH] = useState(cssRowHeight);
  const [range, setRange] = useState({ start: 0, end: count, enabled: false });
  const metrics = useRef({ offset: 0, headerH: 0, viewport: 0 });

  const recompute = useCallback(() => {
    const body = tbodyRef.current;
    const sc = body?.closest('.content-scroll') as HTMLElement | null;
    const all = { start: 0, end: count, enabled: false };
    const set = (r: typeof all) =>
      setRange((p) => (p.start === r.start && p.end === r.end && p.enabled === r.enabled ? p : r));
    if (!body || !sc || count < minRows || !sc.clientHeight) return set(all);

    // Row height is measured, not assumed, so borders/density can never cause drift.
    const first = body.querySelector<HTMLElement>('tr[data-vrow]');
    const measured = first?.getBoundingClientRect().height ?? 0;
    if (measured > 0 && Math.abs(measured - rowH) > 0.25) setRowH(measured);
    const h = measured > 0 ? measured : rowH;

    const offset = body.getBoundingClientRect().top - sc.getBoundingClientRect().top + sc.scrollTop;
    const headerH = body.parentElement?.querySelector('thead')?.getBoundingClientRect().height ?? 0;
    metrics.current = { offset, headerH, viewport: sc.clientHeight };
    const top = sc.scrollTop + headerH - offset;
    const bottom = sc.scrollTop + sc.clientHeight - offset;
    const start = Math.max(0, Math.floor(top / h) - OVERSCAN);
    const end = Math.min(count, Math.ceil(bottom / h) + OVERSCAN);
    set({ start: Math.min(start, count), end: Math.max(end, Math.min(start, count)), enabled: true });
  }, [count, minRows, rowH]);

  useLayoutEffect(() => {
    recompute();
    const sc = tbodyRef.current?.closest('.content-scroll') as HTMLElement | null;
    if (!sc) return;
    sc.addEventListener('scroll', recompute, { passive: true });
    window.addEventListener('resize', recompute);
    const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(recompute) : null;
    ro?.observe(sc);
    return () => {
      sc.removeEventListener('scroll', recompute);
      window.removeEventListener('resize', recompute);
      ro?.disconnect();
    };
  }, [recompute]);

  const scrollToIndex = useCallback(
    (index: number) => {
      const body = tbodyRef.current;
      const sc = body?.closest('.content-scroll') as HTMLElement | null;
      if (!body || !sc || !metrics.current.viewport) return;
      const { offset, headerH, viewport } = metrics.current;
      const top = offset + index * rowH;
      if (top < sc.scrollTop + headerH) sc.scrollTop = Math.max(0, top - headerH);
      else if (top + rowH > sc.scrollTop + viewport) sc.scrollTop = top + rowH - viewport;
      recompute();
    },
    [rowH, recompute],
  );

  const enabled = range.enabled && count >= minRows;
  const start = enabled ? Math.min(range.start, count) : 0;
  const end = enabled ? Math.min(range.end, count) : count;
  return {
    tbodyRef,
    start,
    end,
    topPad: enabled ? start * rowH : 0,
    bottomPad: enabled ? (count - end) * rowH : 0,
    enabled,
    scrollToIndex,
  };
}

/** Empty spacer row that stands in for the rows that are not rendered. */
export function SpacerRow({ height, cols }: { height: number; cols: number }) {
  if (height <= 0) return null;
  return (
    <tr aria-hidden="true" style={{ height }}>
      <td colSpan={cols} style={{ padding: 0, border: 0, height }} />
    </tr>
  );
}
