import { Icon, type IconName } from './Icon';
import { extOf } from '../lib/format';

export function iconFor(kind: string, name: string, mime?: string | null): IconName {
  if (kind === 'folder') return 'folder';
  if (kind === 'note') return 'note';
  if (kind === 'link') return 'link';
  const e = extOf(name);
  if (mime?.startsWith('image/') || ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp'].includes(e))
    return 'image';
  if (e === 'pdf') return 'pdf';
  if (['csv', 'tsv', 'xlsx'].includes(e)) return 'table';
  if (
    [
      'json',
      'js',
      'ts',
      'tsx',
      'rs',
      'py',
      'html',
      'css',
      'toml',
      'yaml',
      'yml',
      'xml',
      'sh',
      'ps1',
    ].includes(e)
  )
    return 'code';
  return 'file';
}
export function FileIcon({
  kind,
  name,
  mime,
  size = 16,
}: {
  kind: string;
  name: string;
  mime?: string | null;
  size?: number;
}) {
  return <Icon name={iconFor(kind, name, mime)} size={size} className={`ficon ${kind}`} />;
}
