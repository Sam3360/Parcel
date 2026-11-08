import type { ReactNode } from 'react';
import { Fragment } from 'react';

/** Small, safe Markdown renderer. Produces React elements only (no raw HTML is ever injected),
 *  and only http/https/mailto links are made clickable. */
const SAFE_URL = /^(https?:\/\/|mailto:)/i;

function inline(text: string, keyBase: string, onLink?: (url: string) => void): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*\s][^*]*\*)|(\[[^\]]+\]\([^)\s]+\))/g;
  let last = 0,
    m: RegExpExecArray | null,
    i = 0;
  while ((m = re.exec(text))) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const tok = m[0],
      key = `${keyBase}-${i++}`;
    if (m[1]) out.push(<code key={key}>{tok.slice(1, -1)}</code>);
    else if (m[2]) out.push(<strong key={key}>{tok.slice(2, -2)}</strong>);
    else if (m[3]) out.push(<em key={key}>{tok.slice(1, -1)}</em>);
    else {
      const lm = /^\[([^\]]+)\]\(([^)\s]+)\)$/.exec(tok)!;
      if (SAFE_URL.test(lm[2])) {
        out.push(
          <a
            key={key}
            href="#"
            onClick={(e) => {
              e.preventDefault();
              onLink?.(lm[2]);
            }}
            title={lm[2]}
          >
            {lm[1]}
          </a>,
        );
      } else out.push(lm[1]);
    }
    last = m.index + tok.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Markdown({ source, onLink }: { source: string; onLink?: (url: string) => void }) {
  const lines = source.replace(/\r\n?/g, '\n').split('\n');
  const blocks: ReactNode[] = [];
  let i = 0,
    k = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (/^```/.test(line)) {
      const buf: string[] = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) buf.push(lines[i++]);
      i++;
      blocks.push(
        <pre key={k++}>
          <code>{buf.join('\n')}</code>
        </pre>,
      );
    } else if (/^#{1,6}\s/.test(line)) {
      const level = line.match(/^#+/)![0].length;
      const Tag = `h${level}` as 'h1';
      blocks.push(<Tag key={k++}>{inline(line.replace(/^#+\s+/, ''), `h${k}`, onLink)}</Tag>);
      i++;
    } else if (/^\s*([-*])\s+/.test(line) || /^\s*\d+\.\s+/.test(line)) {
      const ordered = /^\s*\d+\./.test(line);
      const items: ReactNode[] = [];
      while (i < lines.length && (ordered ? /^\s*\d+\.\s+/.test(lines[i]) : /^\s*[-*]\s+/.test(lines[i]))) {
        const t = lines[i].replace(/^\s*([-*]|\d+\.)\s+/, '');
        const task = /^\[( |x|X)\]\s+(.*)$/.exec(t);
        items.push(
          <li key={items.length}>
            {task ? (
              <>
                <input type="checkbox" checked={task[1] !== ' '} readOnly aria-label="task" />{' '}
                {inline(task[2], `l${k}${items.length}`, onLink)}
              </>
            ) : (
              inline(t, `l${k}${items.length}`, onLink)
            )}
          </li>,
        );
        i++;
      }
      blocks.push(ordered ? <ol key={k++}>{items}</ol> : <ul key={k++}>{items}</ul>);
    } else if (/^>\s?/.test(line)) {
      const buf: string[] = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) buf.push(lines[i++].replace(/^>\s?/, ''));
      blocks.push(<blockquote key={k++}>{inline(buf.join(' '), `q${k}`, onLink)}</blockquote>);
    } else if (/^\s*---+\s*$/.test(line)) {
      blocks.push(<hr key={k++} />);
      i++;
    } else if (line.trim() === '') i++;
    else {
      const buf: string[] = [];
      while (
        i < lines.length &&
        lines[i].trim() !== '' &&
        !/^(#{1,6}\s|```|>\s?|\s*[-*]\s+|\s*\d+\.\s+)/.test(lines[i])
      )
        buf.push(lines[i++]);
      blocks.push(
        <p key={k++}>
          {buf.map((b, j) => (
            <Fragment key={j}>
              {j > 0 && <br />}
              {inline(b, `p${k}-${j}`, onLink)}
            </Fragment>
          ))}
        </p>,
      );
    }
  }
  return <div className="md">{blocks}</div>;
}
