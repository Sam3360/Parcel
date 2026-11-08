import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { Icon, type IconName } from './Icon';

interface BtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'primary' | 'danger' | 'ghost';
  icon?: IconName;
  solid?: boolean;
}
export function Button({
  variant = 'default',
  icon,
  solid,
  className = '',
  children,
  type = 'button',
  ...rest
}: BtnProps) {
  return (
    <button
      type={type}
      className={`btn ${variant === 'default' ? '' : variant} ${solid ? 'solid' : ''} ${className}`}
      {...rest}
    >
      {icon && <Icon name={icon} />}
      {children}
    </button>
  );
}
export function IconButton({
  icon,
  label,
  className = '',
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string }) {
  return (
    <button
      type="button"
      className={`btn icon ghost ${className}`}
      aria-label={label}
      title={label}
      {...rest}
    >
      <Icon name={icon} />
    </button>
  );
}
export function Spinner({ label = 'Loading' }: { label?: string }) {
  return <span className="spinner" role="status" aria-label={label} />;
}
export function Loading({ label = 'Loading…' }: { label?: string }) {
  return (
    <div className="empty" role="status">
      <Spinner label={label} />
      <span>{label}</span>
    </div>
  );
}
export function EmptyState({
  icon,
  title,
  children,
  action,
}: {
  icon: IconName;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <Icon name={icon} size={30} />
      <h3>{title}</h3>
      {children && <p style={{ maxWidth: 420 }}>{children}</p>}
      {action}
    </div>
  );
}
export function Badge({
  tone,
  children,
  title,
}: {
  tone?: 'ok' | 'warn' | 'danger' | 'accent';
  children: ReactNode;
  title?: string;
}) {
  return (
    <span className={`badge ${tone ?? ''}`} title={title}>
      {children}
    </span>
  );
}
export function Banner({
  tone = 'warn',
  children,
  action,
}: {
  tone?: 'warn' | 'error' | 'ok' | 'info';
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className={`banner ${tone}`} role={tone === 'error' ? 'alert' : 'status'}>
      <Icon name={tone === 'ok' ? 'check' : tone === 'info' ? 'info' : 'alert'} />
      <div className="grow">{children}</div>
      {action}
    </div>
  );
}
export function Field({
  label,
  children,
  hint,
  htmlFor,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
  htmlFor?: string;
}) {
  return (
    <div className="field">
      <label htmlFor={htmlFor}>{label}</label>
      {children}
      {hint && <span className="muted small">{hint}</span>}
    </div>
  );
}
export function ErrorText({ error }: { error: string | null }) {
  return error ? (
    <div className="banner error" role="alert">
      <Icon name="alert" />
      <div>{error}</div>
    </div>
  ) : null;
}
