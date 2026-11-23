export interface ShortcutDef {
  keys: string;
  action: string;
  scope: string;
}
export const SHORTCUTS: ShortcutDef[] = [
  {
    keys: 'Ctrl+1 … Ctrl+6',
    action: 'Go to Home, Workspaces, Open Package, Templates, Activity, Settings',
    scope: 'Global',
  },
  { keys: 'Ctrl+O', action: 'Open a package', scope: 'Global' },
  { keys: 'Ctrl+,', action: 'Open Settings', scope: 'Global' },
  { keys: '?', action: 'Show keyboard shortcuts', scope: 'Global' },
  { keys: 'Ctrl+F', action: 'Focus search', scope: 'Editor' },
  { keys: 'Ctrl+Shift+N', action: 'New folder', scope: 'Editor' },
  { keys: 'Ctrl+N', action: 'New note', scope: 'Editor' },
  { keys: 'Ctrl+Shift+A', action: 'Add files', scope: 'Editor' },
  { keys: 'Ctrl+E', action: 'Export package', scope: 'Editor' },
  { keys: 'Ctrl+A', action: 'Select all', scope: 'Editor' },
  { keys: 'F2', action: 'Rename selected item', scope: 'Editor' },
  { keys: 'Ctrl+Z', action: 'Undo the last removal from the workspace', scope: 'Editor' },
  { keys: 'Delete', action: 'Remove selected from workspace (originals untouched)', scope: 'Editor' },
  { keys: 'Enter', action: 'Open folder / edit note', scope: 'Editor' },
  { keys: 'Backspace', action: 'Go up one folder', scope: 'Editor' },
  { keys: '↑ ↓', action: 'Move selection', scope: 'Editor' },
  { keys: 'Shift+Click / Ctrl+Click', action: 'Extend or toggle selection', scope: 'Editor' },
  { keys: 'Shift+F10 / Menu key', action: 'Open context menu', scope: 'Editor' },
  { keys: 'Esc', action: 'Close dialog, menu or clear selection', scope: 'Global' },
];
export function isTyping(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  return (
    !!el &&
    (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable)
  );
}
