import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { baseName, extOf, formatBytes, parentPath, parseCsv, plural, relativeTime } from './format';
import { Markdown } from './markdown';

describe('format helpers', () => {
  it('formats bytes', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 ** 3)).toBe('5.0 GB');
    expect(formatBytes(-1)).toBe('—');
  });
  it('handles paths and plurals', () => {
    expect(extOf('a/b.TXT')).toBe('txt');
    expect(extOf('.gitignore')).toBe('');
    expect(baseName('C:\\x\\y.png')).toBe('y.png');
    expect(parentPath('a/b/c')).toBe('a/b');
    expect(parentPath('c')).toBe('');
    expect(plural(1, 'file')).toBe('1 file');
    expect(plural(2, 'entry', 'entries')).toBe('2 entries');
  });
  it('describes relative time', () => {
    const now = Date.parse('2026-01-01T12:00:00Z');
    expect(relativeTime('2026-01-01T11:59:50Z', now)).toBe('just now');
    expect(relativeTime('2026-01-01T11:00:00Z', now)).toBe('1 h ago');
  });
  it('parses CSV with quotes and newlines', () => {
    expect(parseCsv('a,b\n"x,1","he said ""hi"""\n')).toEqual([
      ['a', 'b'],
      ['x,1', 'he said "hi"'],
    ]);
    expect(parseCsv('a\r\nb')).toEqual([['a'], ['b']]);
    expect(parseCsv('"multi\nline",2')).toEqual([['multi\nline', '2']]);
  });
});

describe('Markdown renderer safety', () => {
  it('never injects raw HTML', () => {
    const { container } = render(
      <Markdown
        source={'<script>window.__pwned=1</script><img src=x onerror=alert(1)>\n\n**bold** and `code`'}
      />,
    );
    expect(container.querySelector('script')).toBeNull();
    expect(container.querySelector('img')).toBeNull();
    expect(container.textContent).toContain('<script>');
    expect(container.querySelector('strong')?.textContent).toBe('bold');
    expect((window as unknown as Record<string, unknown>).__pwned).toBeUndefined();
  });
  it('only makes safe links clickable', async () => {
    const onLink = vi.fn();
    render(
      <Markdown
        source={'[ok](https://example.com) and [bad](javascript:alert(1)) and [file](file:///etc/passwd)'}
        onLink={onLink}
      />,
    );
    const links = screen.getAllByRole('link');
    expect(links).toHaveLength(1);
    expect(links[0]).toHaveAttribute('href', '#');
    links[0].click();
    expect(onLink).toHaveBeenCalledWith('https://example.com');
  });
  it('renders headings, lists, task items, code blocks and quotes', () => {
    const { container } = render(
      <Markdown source={'# Title\n\n- one\n- [x] done\n\n1. first\n\n```\nlet x = 1\n```\n\n> quoted'} />,
    );
    expect(container.querySelector('h1')?.textContent).toBe('Title');
    expect(container.querySelectorAll('ul li')).toHaveLength(2);
    expect(container.querySelector('input[type=checkbox]')).toBeChecked();
    expect(container.querySelector('ol li')?.textContent).toBe('first');
    expect(container.querySelector('pre code')?.textContent).toBe('let x = 1');
    expect(container.querySelector('blockquote')?.textContent).toBe('quoted');
  });
});

describe('isPackagePath', () => {
  it('accepts .parcel files on either path style, case-insensitively', async () => {
    const { isPackagePath } = await import('./format');
    expect(isPackagePath('C:\\Users\\me\\My Pack.parcel')).toBe(true);
    expect(isPackagePath('/home/me/x.PARCEL')).toBe(true);
    expect(isPackagePath('C:\\x\\notes.txt')).toBe(false);
    expect(isPackagePath('C:\\x\\.parcel')).toBe(false);
    expect(isPackagePath('C:\\x\\a.parcel.zip')).toBe(false);
  });
});
