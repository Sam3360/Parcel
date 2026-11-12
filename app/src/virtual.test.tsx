import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderApp } from './test/helpers';
import { makeEntry, samplePackage } from './test/mockApi';
import type { Workspace } from './lib/types';

const W: Workspace = {
  id: 'w1',
  name: 'Big',
  description: '',
  notes: '',
  metadata: {},
  package_id: 'pk',
  revision: 0,
  export_profile_id: null,
  template_id: null,
  last_export_path: null,
  created_at: '',
  modified_at: new Date().toISOString(),
  opened_at: null,
  deleted_at: null,
};
const N = 3000;
const name = (i: number) => `f${String(i).padStart(4, '0')}.txt`;
const bigEntries = (n = N) =>
  Array.from({ length: n }, (_, i) =>
    makeEntry({
      workspace_id: 'w1',
      name: name(i),
      kind: 'file',
      source_path: `/s/${name(i)}`,
      size: 100 + i,
      sha256: `h${i}`,
      mime: 'text/plain',
    }),
  );
const openBig = (n = N) =>
  renderApp({
    route: { page: 'workspaces', workspaceId: 'w1' },
    seed: { workspaces: [{ ...W }], entries: bigEntries(n) },
  });
const table = () => screen.findByRole('table', { name: 'Workspace contents' });
const dataRows = (t: HTMLElement) =>
  within(t)
    .getAllByRole('row')
    .filter((r) => r.hasAttribute('data-entry-id'));
const scroller = (t: HTMLElement) => t.closest('.content-scroll') as HTMLElement;
const scrollTo = (t: HTMLElement, px: number) => {
  scroller(t).scrollTop = px;
  fireEvent.scroll(scroller(t));
};

// jsdom has no layout: give the scroll container a viewport so windowing activates (row height defaults to 32px).
// jsdom also reports every rect as zero, whereas in a browser the table body's top moves up as the container scrolls.
// Emulate that, so the offset arithmetic in useVirtualRows is exercised. (Real-browser layout is NOT covered here.)
vi.setConfig({ testTimeout: 60_000 });
beforeEach(() => {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const sc = this.closest('.content-scroll') as HTMLElement | null;
    const top = this.tagName === 'TBODY' && sc ? -sc.scrollTop : 0;
    return {
      top,
      bottom: top,
      left: 0,
      right: 0,
      width: 0,
      height: 0,
      x: 0,
      y: top,
      toJSON: () => ({}),
    } as DOMRect;
  });
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockImplementation(function (this: HTMLElement) {
    return this.classList.contains('content-scroll') ? 320 : 0;
  });
});
afterEach(() => vi.restoreAllMocks());

describe('list virtualization', () => {
  it('renders only a window of a very large folder', async () => {
    openBig();
    const t = await table();
    const rows = dataRows(t);
    expect(rows.length).toBeGreaterThan(5);
    expect(rows.length).toBeLessThan(80);
    expect(t).toHaveAttribute('aria-rowcount', String(N + 1));
    expect(rows[0]).toHaveAttribute('data-entry-id');
    expect(within(t).getByText(name(0))).toBeInTheDocument();
    expect(within(t).queryByText(name(N - 1))).toBeNull();
  });

  it('shows the right rows after scrolling, with spacers keeping the scroll height', async () => {
    openBig();
    const t = await table();
    scrollTo(t, 32 * 1500);
    await waitFor(() => expect(within(t).queryByText(name(1500))).toBeInTheDocument());
    expect(within(t).queryByText(name(0))).toBeNull();
    expect(dataRows(t).length).toBeLessThan(80);
    const spacers = Array.from(t.querySelectorAll('tbody > tr[aria-hidden]')) as HTMLElement[];
    const total = spacers.reduce((n, s) => n + parseFloat(s.style.height), 0) + dataRows(t).length * 32;
    expect(total).toBe(N * 32);
  });

  it('keeps sorting working: clicking Size twice puts the largest first', async () => {
    const { user } = openBig();
    const t = await table();
    await user.click(within(t).getByRole('button', { name: 'Size' }));
    await user.click(within(t).getByRole('button', { name: 'Size' }));
    const first = dataRows(await table())[0];
    expect(within(first).getByText(name(N - 1))).toBeInTheDocument();
    expect(scroller(t).scrollTop).toBe(0);
  });

  it('keeps search and filtering working on a large workspace', async () => {
    const { user } = openBig();
    await table();
    await user.type(screen.getByLabelText('Search workspace'), 'f0042');
    expect(await screen.findByText('1 result', { exact: false })).toBeInTheDocument();
    const t = await table();
    expect(dataRows(t)).toHaveLength(1);
    expect(t.querySelector('tbody > tr[aria-hidden]')).toBeNull();
    await user.clear(screen.getByLabelText('Search workspace'));
    await waitFor(async () => expect(dataRows(await table()).length).toBeGreaterThan(5));
    expect(dataRows(await table()).length).toBeLessThan(80);
  });

  it('select-all selects every item, including ones that are not rendered', async () => {
    const { user } = openBig();
    await table();
    await user.keyboard('{Control>}a{/Control}');
    expect(await screen.findByText(`${N} items selected`)).toBeInTheDocument();
    expect(screen.getAllByText(`${N} selected`).length).toBeGreaterThan(0); // status bar + live region
  });

  it('shift-click selects a range across rows that were scrolled out of the DOM', async () => {
    const { user } = openBig();
    const t = await table();
    await user.click(within(t).getByText(name(0)));
    scrollTo(t, 32 * 2000);
    const target = await within(t).findByText(name(2000));
    await user.keyboard('{Shift>}');
    await user.click(target);
    await user.keyboard('{/Shift}');
    expect(await screen.findByText('2001 items selected')).toBeInTheDocument();
  });

  it('keyboard navigation follows the selection past the rendered window', async () => {
    const { user } = openBig();
    const t = await table();
    await user.click(within(t).getByText(name(0)));
    await user.keyboard('{ArrowDown}'.repeat(150));
    expect(await screen.findByRole('heading', { name: name(150) })).toBeInTheDocument(); // single selection shown in the inspector
    const row = within(await table())
      .getByText(name(150))
      .closest('tr')!;
    expect(row).toHaveAttribute('aria-selected', 'true');
    expect(dataRows(await table()).length).toBeLessThan(80);
    await user.keyboard('{ArrowUp}'.repeat(10));
    expect(await screen.findByRole('heading', { name: name(140) })).toBeInTheDocument();
    expect(
      within(await table())
        .getByText(name(140))
        .closest('tr'),
    ).toHaveAttribute('aria-selected', 'true');
  });

  it('Delete still removes the selected item from a virtualized list', async () => {
    const { user, state } = openBig(200);
    const t = await table();
    await user.click(within(t).getByText(name(0)));
    await user.keyboard('{Delete}');
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(state.entries).toHaveLength(199));
    await waitFor(async () => expect(within(await table()).queryByText(name(0))).toBeNull());
  });

  it('does not virtualize small lists', async () => {
    openBig(40);
    const t = await table();
    expect(dataRows(t)).toHaveLength(40);
    expect(t.querySelector('tbody > tr[aria-hidden]')).toBeNull();
  });
});

describe('inspector in a large workspace', () => {
  it('does not mount one option per workspace item when an item is selected', async () => {
    const { user } = openBig();
    const t = await table();
    await user.click(within(t).getByText(name(5)));
    const insp = (await screen.findAllByRole('complementary', { name: 'Inspector' }))[0];
    await within(insp).findByRole('heading', { name: name(5) });
    expect(insp.querySelectorAll('option').length).toBeLessThan(10);
    expect(insp.querySelectorAll('*').length).toBeLessThan(250);
  });

  it('lets you filter the picker and caps how many matches it renders', async () => {
    const { user, state } = openBig();
    const t = await table();
    await user.click(within(t).getByText(name(5)));
    const insp = (await screen.findAllByRole('complementary', { name: 'Inspector' }))[0];
    await user.click(within(insp).getByRole('button', { name: 'Dependency target' }));
    const dlg = await screen.findByRole('dialog', { name: 'Dependency target' });
    expect(within(dlg).getAllByRole('option')).toHaveLength(100);
    expect(within(dlg).getByText(/Showing the first 100 of 2999 matches/)).toBeInTheDocument();
    await user.type(within(dlg).getByLabelText('Filter items'), 'f0042');
    const opts = within(dlg).getAllByRole('option');
    expect(opts).toHaveLength(1);
    await user.click(opts[0]);
    await user.click(within(insp).getByRole('button', { name: 'Add dependency' }));
    await waitFor(() => expect(state.deps).toHaveLength(1));
    expect(state.deps[0].target).toBe(state.entries.find((e) => e.name === name(42))!.id);
  });
});

describe('grid view cap', () => {
  it('shows a chunk of tiles, reveals more on request, and keyboard navigation reaches beyond the cap', async () => {
    const { user } = openBig(1000);
    await table();
    await user.click(screen.getByRole('button', { name: 'Grid view' }));
    const grid = await screen.findByRole('listbox', { name: 'Workspace contents' });
    expect(within(grid).getAllByRole('option')).toHaveLength(400);
    expect(screen.getByRole('button', { name: /Show 400 more \(600 not shown\)/ })).toBeInTheDocument();
    await user.click(within(grid).getByText(name(0)));
    await user.keyboard('{ArrowDown}'.repeat(410));
    expect(await screen.findByRole('heading', { name: name(410) })).toBeInTheDocument();
    expect(
      within(screen.getByRole('listbox', { name: 'Workspace contents' })).getAllByRole('option').length,
    ).toBe(800);
    await user.click(screen.getByRole('button', { name: /Show 200 more/ }));
    expect(
      within(screen.getByRole('listbox', { name: 'Workspace contents' })).getAllByRole('option'),
    ).toHaveLength(1000);
    expect(screen.queryByRole('button', { name: /more \(/ })).toBeNull();
  });
});

describe('package explorer at scale', () => {
  const bigPackage = () => {
    const p = samplePackage('/pk/Big.parcel');
    const m = p.manifest;
    m.entries = [];
    for (let d = 0; d < 60; d++) {
      m.entries.push({
        id: `d${d}`,
        kind: 'folder',
        path: `Dir${d}`,
        size: 0,
        description: '',
        tags: [],
        metadata: {},
      });
      m.entries.push({
        id: `s${d}`,
        kind: 'folder',
        path: `Dir${d}/Sub`,
        size: 0,
        description: '',
        tags: [],
        metadata: {},
      });
    }
    for (let i = 0; i < 2500; i++) {
      m.entries.push({
        id: `f${i}`,
        kind: 'file',
        path: `Dir0/${name(i)}`,
        size: 10,
        sha256: 'a'.repeat(64),
        mime: 'text/plain',
        description: '',
        tags: [],
        metadata: {},
      });
    }
    return p;
  };

  it('renders a window of a big folder and only the relevant folder outline', async () => {
    const pkg = bigPackage();
    const { user } = renderApp({
      route: { page: 'open', path: pkg.path },
      seed: { packages: { [pkg.path]: pkg } },
    });
    const folders = await screen.findByRole('complementary', { name: 'Folders' });
    // Top-level folders only until you go somewhere: Dir0..Dir59, but none of the Sub folders.
    expect(within(folders).getAllByRole('button').length).toBe(61);
    await user.dblClick(
      within(await screen.findByRole('table', { name: 'Package entries' })).getByText('Dir0'),
    );
    const t = await screen.findByRole('table', { name: 'Package entries' });
    await waitFor(() => expect(within(t).getAllByRole('row').length).toBeLessThan(90));
    expect(within(t).getByText(name(0))).toBeInTheDocument();
    expect(t).toHaveAttribute('aria-rowcount', '2502'); // 2500 files + Sub + header
    expect(within(folders).getByText('Sub')).toBeInTheDocument(); // children of the current folder now appear
    scrollTo(t, 32 * 1200);
    await waitFor(() => expect(within(t).queryByText(name(1200))).toBeInTheDocument());
    expect(within(t).queryByText(name(0))).toBeNull();
  });
});
