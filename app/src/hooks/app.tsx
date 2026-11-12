import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { describeError, type Api } from '../lib/api';
import type { Progress } from '../lib/types';
import { uid } from '../lib/format';
import { ConfirmDialog, Dialog, PromptDialog } from '../components/Dialog';
import { Button } from '../components/ui';
import { Icon } from '../components/Icon';

export type Route =
  | { page: 'home' }
  | { page: 'workspaces'; workspaceId?: string }
  | { page: 'open'; path?: string }
  | { page: 'templates' }
  | { page: 'activity' }
  | { page: 'settings' };

type Tone = 'info' | 'success' | 'error' | 'warn';
interface ToastAction {
  label: string;
  onClick: () => void;
}
interface Toast {
  id: number;
  msg: string;
  tone: Tone;
  action?: ToastAction;
}
interface ConfirmOpts {
  title: string;
  message: ReactNode;
  confirmLabel?: string;
  danger?: boolean;
}
interface PromptOpts {
  title: string;
  label: string;
  initial?: string;
  confirmLabel?: string;
  validate?: (v: string) => string | null;
}

export interface AppCtx {
  api: Api;
  prefs: Record<string, string>;
  setPref: (k: string, v: string) => Promise<void>;
  route: Route;
  navigate: (r: Route) => void;
  toast: (msg: string, tone?: Tone, action?: ToastAction) => void;
  reportError: (e: unknown, context?: string) => void;
  confirm: (o: ConfirmOpts) => Promise<boolean>;
  prompt: (o: PromptOpts) => Promise<string | null>;
  runJob: <T>(title: string, fn: (jobId: string) => Promise<T>) => Promise<T>;
  showShortcuts: () => void;
}

const Ctx = createContext<AppCtx | null>(null);
export function useApp(): AppCtx {
  const c = useContext(Ctx);
  if (!c) throw new Error('useApp outside provider');
  return c;
}

function applyTheme(theme: string, density: string, reduce: string) {
  const root = document.documentElement;
  const dark =
    theme === 'dark' || (theme !== 'light' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  root.dataset.theme = dark ? 'dark' : 'light';
  root.dataset.density = density === 'compact' ? 'compact' : 'comfortable';
  root.dataset.motion = reduce === 'true' ? 'reduce' : 'full';
}

export function AppProvider({
  api,
  children,
  initialRoute = { page: 'home' },
}: {
  api: Api;
  children: ReactNode;
  initialRoute?: Route;
}) {
  const [prefs, setPrefs] = useState<Record<string, string>>({});
  const [route, setRoute] = useState<Route>(initialRoute);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [dialog, setDialog] = useState<ReactNode>(null);
  const [job, setJob] = useState<{
    id: string;
    title: string;
    progress: Progress | null;
    visible: boolean;
    cancelling: boolean;
  } | null>(null);
  const [shortcuts, setShortcuts] = useState(false);
  const seq = useRef(0);

  useEffect(() => {
    api
      .getPrefs()
      .then(setPrefs)
      .catch(() => {});
  }, [api]);
  useEffect(() => {
    applyTheme(prefs.theme ?? 'system', prefs.density ?? 'comfortable', prefs.reduce_motion ?? 'false');
    if ((prefs.theme ?? 'system') !== 'system') return;
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const h = () => applyTheme('system', prefs.density ?? 'comfortable', prefs.reduce_motion ?? 'false');
    mq.addEventListener?.('change', h);
    return () => mq.removeEventListener?.('change', h);
  }, [prefs]);

  const toast = useCallback((msg: string, tone: Tone = 'info', action?: ToastAction) => {
    const id = ++seq.current;
    setToasts((t) => [...t, { id, msg, tone, action }]);
    // Toasts with an action stay a little longer so there is time to use it.
    setTimeout(
      () => setToasts((t) => t.filter((x) => x.id !== id)),
      tone === 'error' ? 9000 : action ? 8000 : 4500,
    );
  }, []);
  const reportError = useCallback(
    (e: unknown, context?: string) => {
      toast(context ? `${context}: ${describeError(e)}` : describeError(e), 'error');
    },
    [toast],
  );
  const setPref = useCallback(
    async (k: string, v: string) => {
      await api.setPref(k, v);
      setPrefs((p) => ({ ...p, [k]: v }));
    },
    [api],
  );

  const confirm = useCallback(
    (o: ConfirmOpts) =>
      new Promise<boolean>((res) => {
        setDialog(
          <ConfirmDialog
            {...o}
            onResolve={(ok) => {
              setDialog(null);
              res(ok);
            }}
          />,
        );
      }),
    [],
  );
  const prompt = useCallback(
    (o: PromptOpts) =>
      new Promise<string | null>((res) => {
        setDialog(
          <PromptDialog
            {...o}
            onResolve={(v) => {
              setDialog(null);
              res(v);
            }}
          />,
        );
      }),
    [],
  );

  const runJob = useCallback(
    async <T,>(title: string, fn: (jobId: string) => Promise<T>): Promise<T> => {
      const id = uid();
      setJob({ id, title, progress: null, visible: false, cancelling: false });
      const showTimer = setTimeout(
        () => setJob((j) => (j && j.id === id ? { ...j, visible: true } : j)),
        200,
      );
      const off = api.onProgress((p) => {
        if (p.job_id === id) setJob((j) => (j && j.id === id ? { ...j, progress: p } : j));
      });
      try {
        return await fn(id);
      } finally {
        clearTimeout(showTimer);
        off();
        setJob(null);
      }
    },
    [api],
  );

  const ctx = useMemo<AppCtx>(
    () => ({
      api,
      prefs,
      setPref,
      route,
      navigate: setRoute,
      toast,
      reportError,
      confirm,
      prompt,
      runJob,
      showShortcuts: () => setShortcuts(true),
    }),
    [api, prefs, setPref, route, toast, reportError, confirm, prompt, runJob],
  );

  return (
    <Ctx.Provider value={ctx}>
      {children}
      {dialog}
      {shortcuts && <ShortcutsDialog onClose={() => setShortcuts(false)} />}
      {job?.visible && (
        <Dialog
          title={job.title}
          onClose={() => {}}
          dismissible={false}
          footer={
            <Button
              disabled={job.cancelling}
              onClick={() => {
                setJob({ ...job, cancelling: true });
                api.cancelJob(job.id).catch(() => {});
              }}
            >
              {job.cancelling ? 'Cancelling…' : 'Cancel'}
            </Button>
          }
        >
          <JobProgress p={job.progress} />
        </Dialog>
      )}
      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.tone}`} role={t.tone === 'error' ? 'alert' : 'status'}>
            <div className="grow">{t.msg}</div>
            {t.action && (
              <button
                className="btn"
                style={{ height: 24 }}
                onClick={() => {
                  t.action?.onClick();
                  setToasts((x) => x.filter((y) => y.id !== t.id));
                }}
              >
                {t.action.label}
              </button>
            )}
            <button
              className="btn icon ghost"
              style={{ height: 22, width: 22 }}
              aria-label="Dismiss"
              onClick={() => setToasts((x) => x.filter((y) => y.id !== t.id))}
            >
              <Icon name="x" size={13} />
            </button>
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

function JobProgress({ p }: { p: Progress | null }) {
  const pct =
    p && p.bytes_total > 0
      ? Math.min(100, (p.bytes_done / p.bytes_total) * 100)
      : p && p.items_total > 0
        ? Math.min(100, (p.items_done / p.items_total) * 100)
        : null;
  return (
    <div className="stack">
      <div className="muted small ellipsis" title={p?.current_item}>
        {p ? `${p.phase}${p.current_item ? ` — ${p.current_item}` : ''}` : 'Working…'}
      </div>
      <div
        className={`progress ${pct === null ? 'indeterminate' : ''}`}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={pct === null ? undefined : Math.round(pct)}
      >
        <div style={pct === null ? undefined : { width: `${pct}%` }} />
      </div>
      {p && p.items_total > 0 && (
        <div className="muted small">
          {p.items_done} of {p.items_total} items
        </div>
      )}
    </div>
  );
}

import { SHORTCUTS } from '../lib/shortcuts';
function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog
      title="Keyboard shortcuts"
      onClose={onClose}
      size="wide"
      footer={
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      }
    >
      <table className="table" aria-label="Keyboard shortcuts">
        <tbody>
          {SHORTCUTS.map((s) => (
            <tr key={s.keys + s.action}>
              <td style={{ width: 200 }}>
                <span className="kbd">{s.keys}</span>
              </td>
              <td>{s.action}</td>
              <td className="muted">{s.scope}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Dialog>
  );
}
