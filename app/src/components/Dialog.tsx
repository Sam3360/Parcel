import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { Button } from './ui';

const FOCUSABLE =
  'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

export function Dialog({
  title,
  onClose,
  children,
  footer,
  size = 'normal',
  dismissible = true,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  size?: 'normal' | 'wide' | 'xwide';
  dismissible?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const id = useId();
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const el = ref.current;
    const first =
      el?.querySelector<HTMLElement>(
        'input:not(:disabled), textarea:not(:disabled), select:not(:disabled)',
      ) ?? el?.querySelector<HTMLElement>(FOCUSABLE);
    first?.focus();
    return () => prev?.focus?.();
  }, []);
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape' && dismissible) {
      e.stopPropagation();
      onClose();
    }
    if (e.key === 'Tab' && ref.current) {
      const f = Array.from(ref.current.querySelectorAll<HTMLElement>(FOCUSABLE));
      if (!f.length) return;
      const [a, z] = [f[0], f[f.length - 1]];
      if (e.shiftKey && document.activeElement === a) {
        e.preventDefault();
        z.focus();
      } else if (!e.shiftKey && document.activeElement === z) {
        e.preventDefault();
        a.focus();
      }
    }
  };
  return (
    <div
      className="overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && dismissible) onClose();
      }}
    >
      <div
        ref={ref}
        className={`dialog ${size === 'normal' ? '' : size}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={id}
        onKeyDown={onKey}
      >
        <header>
          <h2 id={id}>{title}</h2>
        </header>
        <div className="body">{children}</div>
        {footer && <footer>{footer}</footer>}
      </div>
    </div>
  );
}

export function ConfirmDialog({
  title,
  message,
  confirmLabel = 'Confirm',
  danger,
  onResolve,
}: {
  title: string;
  message: ReactNode;
  confirmLabel?: string;
  danger?: boolean;
  onResolve: (ok: boolean) => void;
}) {
  return (
    <Dialog
      title={title}
      onClose={() => onResolve(false)}
      footer={
        <>
          <Button onClick={() => onResolve(false)}>Cancel</Button>
          <Button variant={danger ? 'danger' : 'primary'} solid={danger} onClick={() => onResolve(true)}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="stack">{message}</div>
    </Dialog>
  );
}

export function PromptDialog({
  title,
  label,
  initial = '',
  confirmLabel = 'OK',
  validate,
  onResolve,
}: {
  title: string;
  label: string;
  initial?: string;
  confirmLabel?: string;
  validate?: (v: string) => string | null;
  onResolve: (v: string | null) => void;
}) {
  const [value, setValue] = useState(initial);
  const err = validate ? validate(value) : null;
  const submit = () => {
    if (!err && value.trim()) onResolve(value.trim());
  };
  return (
    <Dialog
      title={title}
      onClose={() => onResolve(null)}
      footer={
        <>
          <Button onClick={() => onResolve(null)}>Cancel</Button>
          <Button variant="primary" disabled={!!err || !value.trim()} onClick={submit}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="field">
          <label htmlFor="prompt-input">{label}</label>
          <input
            id="prompt-input"
            className="input"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            autoComplete="off"
            spellCheck={false}
            onFocus={(e) => e.currentTarget.select()}
            aria-invalid={!!err}
            aria-describedby={err ? 'prompt-err' : undefined}
          />
          {err && value && (
            <span id="prompt-err" className="small" style={{ color: 'var(--danger)' }}>
              {err}
            </span>
          )}
        </div>
      </form>
    </Dialog>
  );
}
