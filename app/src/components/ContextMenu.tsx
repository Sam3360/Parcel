import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Icon, type IconName } from './Icon';

export interface MenuItem {
  label?: string;
  icon?: IconName;
  onSelect?: () => void;
  danger?: boolean;
  disabled?: boolean;
  shortcut?: string;
  separator?: boolean;
}

export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (r)
      setPos({
        x: Math.max(4, Math.min(x, window.innerWidth - r.width - 4)),
        y: Math.max(4, Math.min(y, window.innerHeight - r.height - 4)),
      });
  }, [x, y]);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>('button:not(:disabled)')?.focus();
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const b = Array.from(ref.current?.querySelectorAll<HTMLElement>('button:not(:disabled)') ?? []);
        const i = b.indexOf(document.activeElement as HTMLElement);
        b[(i + (e.key === 'ArrowDown' ? 1 : -1) + b.length) % b.length]?.focus();
      }
    };
    window.addEventListener('mousedown', down, true);
    window.addEventListener('keydown', key, true);
    window.addEventListener('blur', onClose);
    return () => {
      window.removeEventListener('mousedown', down, true);
      window.removeEventListener('keydown', key, true);
      window.removeEventListener('blur', onClose);
    };
  }, [onClose]);
  return (
    <div ref={ref} className="menu" role="menu" style={{ left: pos.x, top: pos.y }}>
      {items.map((it, i) =>
        it.separator ? (
          <hr key={i} />
        ) : (
          <button
            key={i}
            role="menuitem"
            disabled={it.disabled}
            className={it.danger ? 'danger' : ''}
            onClick={() => {
              onClose();
              it.onSelect?.();
            }}
          >
            {it.icon ? <Icon name={it.icon} /> : <span style={{ width: 16 }} />}
            {it.label}
            {it.shortcut && <span className="kbd">{it.shortcut}</span>}
          </button>
        ),
      )}
    </div>
  );
}
