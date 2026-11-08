const P: Record<string, string> = {
  home: 'M3 11l9-8 9 8M5 10v10h14V10M10 20v-6h4v6',
  layers: 'M12 3l9 5-9 5-9-5 9-5zM3 13l9 5 9-5M3 17.5l9 5 9-5',
  package: 'M3 8l9-5 9 5v8l-9 5-9-5V8zM3 8l9 5 9-5M12 13v8',
  template: 'M4 4h16v6H4zM4 14h7v6H4zM15 14h5v6h-5z',
  activity: 'M3 12h4l3-8 4 16 3-8h4',
  settings:
    'M12 15a3 3 0 100-6 3 3 0 000 6zM19 12a7 7 0 00-.1-1.2l2-1.5-2-3.4-2.3.9a7 7 0 00-2-1.2L14.2 3h-4l-.4 2.6a7 7 0 00-2 1.2l-2.3-.9-2 3.4 2 1.5a7 7 0 000 2.4l-2 1.5 2 3.4 2.3-.9a7 7 0 002 1.2l.4 2.6h4l.4-2.6a7 7 0 002-1.2l2.3.9 2-3.4-2-1.5c.1-.4.1-.8.1-1.2z',
  folder: 'M3 6a2 2 0 012-2h4l2 2h8a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V6z',
  file: 'M6 3h8l5 5v13H6V3zM14 3v5h5',
  note: 'M5 4h14v16H5V4zM8 9h8M8 13h8M8 17h5',
  link: 'M10 14a4 4 0 005.7 0l3-3a4 4 0 00-5.7-5.7l-1 1M14 10a4 4 0 00-5.7 0l-3 3A4 4 0 0011 18.7l1-1',
  image: 'M4 5h16v14H4V5zM4 16l5-5 4 4 3-3 4 4M9 9.5a1 1 0 100-.01',
  search: 'M11 18a7 7 0 100-14 7 7 0 000 14zM21 21l-5-5',
  grid: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z',
  list: 'M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01',
  plus: 'M12 5v14M5 12h14',
  trash: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  right: 'M9 6l6 6-6 6',
  down: 'M6 9l6 6 6-6',
  check: 'M5 12l5 5 9-10',
  alert: 'M12 4l10 17H2L12 4zM12 10v5M12 18h.01',
  x: 'M6 6l12 12M18 6L6 18',
  download: 'M12 4v11M7 11l5 5 5-5M5 20h14',
  upload: 'M12 16V5M7 9l5-5 5 5M5 20h14',
  external: 'M14 4h6v6M20 4l-9 9M18 14v5H5V6h5',
  shield: 'M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6l8-3zM9 12l2 2 4-4',
  copy: 'M9 9h11v11H9V9zM5 15V4h11',
  refresh: 'M20 11a8 8 0 00-14-4L4 9M4 4v5h5M4 13a8 8 0 0014 4l2-2M20 20v-5h-5',
  edit: 'M4 20h4L19 9l-4-4L4 16v4zM13 7l4 4',
  sun: 'M12 16a4 4 0 100-8 4 4 0 000 8zM12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M5 19l1.5-1.5M17.5 6.5L19 5',
  moon: 'M20 14.5A8 8 0 019.5 4 8 8 0 1020 14.5z',
  table: 'M3 5h18v14H3V5zM3 10h18M9 5v14',
  code: 'M8 8l-5 4 5 4M16 8l5 4-5 4M14 5l-4 14',
  pdf: 'M6 3h8l5 5v13H6V3zM14 3v5h5M9 15h6M9 18h4',
  box: 'M12 2.5l8.5 4.7v9.6L12 21.5l-8.5-4.7V7.2L12 2.5zM3.5 7.2L12 12l8.5-4.8M12 12v9.5',
  tag: 'M3 12V4h8l10 10-8 8L3 12zM7.5 8h.01',
  compare: 'M7 4v16M17 4v16M3 8l4-4 4 4M13 16l4 4 4-4',
  info: 'M12 21a9 9 0 100-18 9 9 0 000 18zM12 11v6M12 7.5h.01',
  keyboard: 'M3 6h18v12H3V6zM7 10h.01M11 10h.01M15 10h.01M7 14h10',
  star: 'M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1L3.2 9.5l6.1-.9L12 3z',
};

export type IconName = keyof typeof P;

export function Icon({
  name,
  size = 16,
  className = '',
  title,
}: {
  name: IconName;
  size?: number;
  className?: string;
  title?: string;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
      aria-label={title}
      focusable="false"
    >
      <path d={P[name]} />
    </svg>
  );
}
