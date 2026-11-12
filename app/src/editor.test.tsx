import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from './test/helpers';
import { makeEntry, type MockState } from './test/mockApi';
import type { Entry, Workspace } from './lib/types';

const W: Workspace = {
  id: 'w1',
  name: 'Project',
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
const file = (name: string, extra: Partial<Entry> = {}) =>
  makeEntry({
    workspace_id: 'w1',
    name,
    kind: 'file',
    source_path: `/src/${name}`,
    size: 1000,
    sha256: `h-${name}`,
    mime: 'text/plain',
    ...extra,
  });
const open = (seed: Partial<MockState> = {}, api = {}) =>
  renderApp({
    route: { page: 'workspaces', workspaceId: 'w1' },
    seed: { workspaces: [{ ...W }], ...seed },
    api,
  });
const itemsTable = () => screen.findByRole('table', { name: 'Workspace contents' });
const row = async (name: string) => (await within(await itemsTable()).findByText(name)).closest('tr')!;

describe('workspace editor layout', () => {
  it('renders the three-panel layout with structure, contents and inspector', async () => {
    open({ entries: [makeEntry({ workspace_id: 'w1', name: 'Docs', kind: 'folder' }), file('a.txt')] });
    expect(await screen.findByRole('complementary', { name: 'Navigation' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Contents' })).toBeInTheDocument();
    expect(screen.getAllByRole('complementary', { name: 'Inspector' }).length).toBeGreaterThan(0);
    expect(await screen.findByLabelText('Workspace overview')).toBeInTheDocument();
    expect(screen.getByRole('tree', { name: 'Workspace structure' })).toBeInTheDocument();
  });

  it('shows an error state when the workspace cannot be opened', async () => {
    renderApp({ route: { page: 'workspaces', workspaceId: 'missing' } });
    expect(await screen.findByText('Could not open this workspace')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent(/not found/);
  });

  it('shows statistics based on actual data', async () => {
    const { user } = open({
      entries: [
        makeEntry({ workspace_id: 'w1', name: 'F', kind: 'folder' }),
        file('a.txt', { size: 2048 }),
        file('b.txt', { size: 2048 }),
        makeEntry({ workspace_id: 'w1', name: 'n.md', kind: 'note', size: 5 }),
      ],
    });
    const stats = await screen.findByRole('list', { hidden: true }).catch(() => null);
    void stats;
    void user;
    const dl = await screen.findByLabelText('Workspace statistics');
    expect(within(dl).getByText('Files').nextSibling).toHaveTextContent('2');
    expect(within(dl).getByText('Folders').nextSibling).toHaveTextContent('1');
    expect(within(dl).getByText('Notes').nextSibling).toHaveTextContent('1');
    expect(within(dl).getByText('Total size').nextSibling).toHaveTextContent('4.0 KB');
  });
});

describe('adding and removing entries', () => {
  it('adds files chosen through the picker, from different locations', async () => {
    const { user, state } = open();
    state.picks.files = ['/Documents/thesis.txt', '/Downloads/figure.png'];
    expect(await screen.findByText('Add your first items')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add files…' }));
    expect(await row('thesis.txt')).toBeInTheDocument();
    expect(await row('figure.png')).toBeInTheDocument();
    expect(state.entries.map((e) => e.source_path)).toEqual([
      '/Documents/thesis.txt',
      '/Downloads/figure.png',
    ]);
    expect(await screen.findByText(/Added 2 files/)).toBeInTheDocument();
  });

  it('adds dropped files into the current folder', async () => {
    const { state } = open({ entries: [] });
    await screen.findByText('Add your first items');
    state.dropHandlers.forEach((h) => h(['/x/dropped.txt']));
    await waitFor(() => expect(state.entries.map((e) => e.name)).toEqual(['dropped.txt']));
  });

  it('does not treat a dropped .parcel as an "open package" request inside a workspace', async () => {
    const { state } = open({ entries: [] });
    await screen.findByText('Add your first items');
    state.dropHandlers.forEach((h) => h(['/x/Other.parcel']));
    await waitFor(() => expect(state.entries.map((e) => e.name)).toEqual(['Other.parcel']));
    expect(screen.getByRole('toolbar', { name: 'Workspace tools' })).toBeInTheDocument();
  });

  it('reports files that were already collected instead of duplicating them', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    state.picks.files = ['/src/a.txt'];
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Add' }));
    await user.click(screen.getByRole('menuitem', { name: /Files…/ }));
    expect(await screen.findByText(/1 already in this workspace/)).toBeInTheDocument();
    expect(state.entries).toHaveLength(1);
  });

  it('removes entries from the workspace only after confirmation, never touching originals', async () => {
    const { user, state } = open({ entries: [file('a.txt'), file('b.txt')] });
    await user.click(await row('a.txt'));
    await user.keyboard('{Delete}');
    const dlg = await screen.findByRole('dialog');
    expect(within(dlg).getByText(/Your original files are not deleted or changed/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole('button', { name: 'Cancel' }));
    expect(state.entries).toHaveLength(2);
    await user.keyboard('{Delete}');
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(state.entries.map((e) => e.name)).toEqual(['b.txt']));
    expect(screen.queryByText('a.txt')).toBeNull();
  });

  it('shows a clear error when an operation fails', async () => {
    const { user, state } = open(
      { entries: [] },
      {
        addPaths: async () => {
          throw new Error('permission denied');
        },
      },
    );
    state.picks.files = ['/x/y.txt'];
    await user.click(await screen.findByRole('button', { name: 'Add files…' }));
    expect(await screen.findByText(/permission denied/)).toBeInTheDocument();
  });
});

describe('folders and organisation', () => {
  it('creates a folder, navigates into it with breadcrumbs, and goes back up', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Add' }));
    await user.click(screen.getByRole('menuitem', { name: /New folder/ }));
    await user.type(screen.getByLabelText('Folder name'), 'Reports{Enter}');
    await waitFor(() => expect(state.entries.some((e) => e.name === 'Reports')).toBe(true));
    await user.dblClick(await row('Reports'));
    const crumbs = await screen.findByLabelText('Location');
    expect(within(crumbs).getByText('Reports')).toBeInTheDocument();
    expect(screen.getByText('This folder is empty')).toBeInTheDocument();
    await user.keyboard('{Backspace}');
    expect(await row('a.txt')).toBeInTheDocument();
  });

  it('rejects clashing and invalid folder names with an explanation', async () => {
    const { user } = open({ entries: [makeEntry({ workspace_id: 'w1', name: 'Docs', kind: 'folder' })] });
    await itemsTable();
    await user.keyboard('{Control>}{Shift>}n{/Shift}{/Control}');
    await user.type(await screen.findByLabelText('Folder name'), 'docs{Enter}');
    expect(await screen.findByText(/already exists here/)).toBeInTheDocument();
  });

  it('renames with F2 and enforces unique sibling names', async () => {
    const { user, state } = open({ entries: [file('a.txt'), file('b.txt')] });
    await user.click(await row('a.txt'));
    await user.keyboard('{F2}');
    const input = await screen.findByLabelText('New name');
    await user.clear(input);
    await user.type(input, 'renamed.txt{Enter}');
    await waitFor(() => expect(state.entries[0].name).toBe('renamed.txt'));
    await user.click(await row('b.txt'));
    await user.keyboard('{F2}');
    const i2 = await screen.findByLabelText('New name');
    await user.clear(i2);
    await user.type(i2, 'RENAMED.TXT{Enter}');
    expect(await screen.findByText(/already exists here/)).toBeInTheDocument();
    expect(state.entries[1].name).toBe('b.txt');
  });

  it('moves several selected entries into a folder through the Move dialog', async () => {
    const { user, state } = open({
      entries: [
        makeEntry({ workspace_id: 'w1', name: 'Archive', kind: 'folder' }),
        file('a.txt'),
        file('b.txt'),
        file('c.txt'),
      ],
    });
    await user.click(await row('a.txt'));
    await user.keyboard('{Shift>}');
    await user.click(await row('b.txt'));
    await user.keyboard('{/Shift}');
    expect(await screen.findByText('2 items selected')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Move to…' }));
    const dlg = await screen.findByRole('dialog');
    await user.click(within(dlg).getByRole('option', { name: 'Archive' }));
    await user.click(within(dlg).getByRole('button', { name: 'Move here' }));
    const archive = state.entries.find((e) => e.name === 'Archive')!;
    await waitFor(() =>
      expect(
        state.entries
          .filter((e) => e.parent_id === archive.id)
          .map((e) => e.name)
          .sort(),
      ).toEqual(['a.txt', 'b.txt']),
    );
    expect(state.entries.find((e) => e.name === 'c.txt')!.parent_id).toBeNull();
  });

  it('supports ctrl-click multi-select and select-all', async () => {
    const { user } = open({ entries: [file('a.txt'), file('b.txt'), file('c.txt')] });
    await user.click(await row('a.txt'));
    await user.keyboard('{Control>}');
    await user.click(await row('c.txt'));
    await user.keyboard('{/Control}');
    expect(await screen.findByText('2 items selected')).toBeInTheDocument();
    await user.keyboard('{Control>}a{/Control}');
    expect(await screen.findByText('3 items selected')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(await screen.findByLabelText('Workspace overview')).toBeInTheDocument();
  });

  it('moves items by dragging them onto a folder in the structure tree', async () => {
    const { state } = open({
      entries: [makeEntry({ workspace_id: 'w1', name: 'Target', kind: 'folder' }), file('drag.txt')],
    });
    const r = await row('drag.txt');
    const tree = screen.getByRole('tree', { name: 'Workspace structure' });
    const target = within(tree).getByRole('button', { name: /Target/ });
    const data: Record<string, string> = {};
    const dt = {
      setData: (k: string, v: string) => {
        data[k] = v;
      },
      getData: (k: string) => data[k] ?? '',
      types: ['application/x-parcel-ids'],
      effectAllowed: '',
    };
    fireEvent.dragStart(r, { dataTransfer: dt });
    fireEvent.dragOver(target, { dataTransfer: dt });
    fireEvent.drop(target, { dataTransfer: dt });
    const id = state.entries.find((e) => e.name === 'Target')!.id;
    await waitFor(() => expect(state.entries.find((e) => e.name === 'drag.txt')!.parent_id).toBe(id));
  });

  it('does not offer a folder or its descendants as a move destination', async () => {
    const loop = makeEntry({ workspace_id: 'w1', name: 'Loop', kind: 'folder' });
    const inner = makeEntry({ workspace_id: 'w1', name: 'Inner', kind: 'folder', parent_id: loop.id });
    const other = makeEntry({ workspace_id: 'w1', name: 'Elsewhere', kind: 'folder' });
    const { user } = open({ entries: [loop, inner, other] });
    await user.click(await row('Loop'));
    await user.keyboard('{Shift>}{F10}{/Shift}');
    await user.click(await screen.findByRole('menuitem', { name: /Move to…/ }));
    const dlg = await screen.findByRole('dialog');
    expect(within(dlg).getByRole('option', { name: 'Loop' })).toBeDisabled();
    expect(within(dlg).getByRole('option', { name: 'Inner' })).toBeDisabled();
    expect(within(dlg).getByRole('option', { name: 'Elsewhere' })).toBeEnabled();
    expect(within(dlg).getByRole('option', { name: 'Workspace root' })).toBeEnabled();
  });
});

describe('notes and references', () => {
  it('creates a note, edits Markdown with live preview and autosaves', async () => {
    const { user, state } = open();
    await screen.findByText('Add your first items');
    await user.keyboard('{Control>}n{/Control}');
    await user.type(await screen.findByLabelText('Note title'), 'Ideas{Enter}');
    const editor = await screen.findByLabelText('Markdown source');
    await user.clear(editor);
    await user.type(editor, '# Heading{Enter}some **bold** text');
    const preview = screen.getByLabelText('Rendered preview');
    expect(within(preview).getByRole('heading', { name: 'Heading' })).toBeInTheDocument();
    expect(preview.querySelector('strong')?.textContent).toBe('bold');
    await waitFor(
      () => expect(state.entries.find((e) => e.name === 'Ideas.md')?.content).toContain('**bold**'),
      { timeout: 3000 },
    );
    expect(await screen.findByText('All changes saved')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(await row('Ideas.md')).toBeInTheDocument();
  });

  it('saves workspace-level notes', async () => {
    const { user, state } = open();
    await user.click(await screen.findByRole('button', { name: 'Edit workspace notes' }));
    await user.type(await screen.findByLabelText('Markdown source'), 'Overall notes');
    await user.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(state.workspaces[0].notes).toBe('Overall notes'));
  });

  it('adds, validates and deletes references, never fetching them', async () => {
    const { user, state } = open();
    await user.click(await screen.findByRole('tab', { name: /References/ }));
    await user.click(await screen.findByRole('button', { name: 'Add reference' }));
    await user.type(screen.getByLabelText('Title'), 'Docs');
    const url = screen.getByLabelText('URL');
    await user.clear(url);
    await user.type(url, 'javascript:alert(1)');
    await user.click(screen.getAllByRole('button', { name: 'Add reference' }).pop()!);
    expect(await screen.findByText(/only http, https, ftp and mailto/)).toBeInTheDocument();
    await user.clear(url);
    await user.type(url, 'https://doc.rust-lang.org');
    await user.type(screen.getByLabelText('Category'), 'Reference');
    await user.click(screen.getAllByRole('button', { name: 'Add reference' }).pop()!);
    expect(await screen.findByText('doc.rust-lang.org', { exact: false })).toBeInTheDocument();
    expect(state.refs).toHaveLength(1);
    await user.click(screen.getByRole('button', { name: 'Delete Docs' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(state.refs).toHaveLength(0));
  });

  it('asks before opening a link in the browser', async () => {
    let opened = '';
    const { user } = open(
      {
        refs: [
          {
            id: 'r',
            workspace_id: 'w1',
            title: 'Rust',
            url: 'https://www.rust-lang.org',
            description: '',
            category: '',
            entry_id: null,
            sort_order: 1,
          },
        ],
      },
      {
        openExternal: async (u: string) => {
          opened = u;
        },
      },
    );
    await user.click(await screen.findByRole('tab', { name: /References/ }));
    await user.click(await screen.findByRole('button', { name: 'Rust' }));
    expect(opened).toBe('');
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Open' }));
    await waitFor(() => expect(opened).toBe('https://www.rust-lang.org'));
  });

  it('adds a link entry and rejects unsafe schemes', async () => {
    const { user, state } = open();
    await screen.findByText('Add your first items');
    await user.click(screen.getByRole('button', { name: 'Add' }));
    await user.click(screen.getByRole('menuitem', { name: /Add link/ }));
    await user.type(screen.getByLabelText('Title'), 'Evil');
    const url = screen.getByLabelText('URL');
    await user.clear(url);
    await user.type(url, 'file:///C:/Windows');
    await user.click(screen.getByRole('button', { name: 'Add link' }));
    expect(await screen.findByText(/only http, https/)).toBeInTheDocument();
    await user.clear(url);
    await user.type(url, 'https://example.com');
    await user.click(screen.getByRole('button', { name: 'Add link' }));
    await waitFor(() => expect(state.entries.map((e) => e.kind)).toEqual(['link']));
  });
});

describe('inspector, metadata and descriptions', () => {
  it('edits description, tags and custom metadata with autosave', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    await user.click(await row('a.txt'));
    const insp = await screen.findByLabelText('Inspector', { selector: 'div' });
    expect(within(insp).getByText('/src/a.txt')).toBeInTheDocument();
    expect(await within(insp).findByText('contents of a.txt')).toBeInTheDocument();
    const desc = within(insp).getByLabelText('Description');
    await user.type(desc, 'Quarterly report');
    await user.tab();
    await waitFor(() => expect(state.entries[0].description).toBe('Quarterly report'));
    await user.type(within(insp).getByLabelText('Add tag'), 'finance{Enter}');
    await waitFor(() => expect(state.entries[0].tags).toEqual(['finance']));
    await user.type(within(insp).getByLabelText('Metadata key'), 'author');
    await user.type(within(insp).getByLabelText('Metadata value'), 'Sam{Enter}');
    await waitFor(() => expect(state.entries[0].metadata).toEqual({ author: 'Sam' }));
  });

  it('adds batch tags to multiple selected entries', async () => {
    const { user, state } = open({ entries: [file('a.txt'), file('b.txt')] });
    await user.click(await row('a.txt'));
    await user.keyboard('{Control>}');
    await user.click(await row('b.txt'));
    await user.keyboard('{/Control}');
    await user.click(await screen.findByRole('button', { name: 'Add tag…' }));
    await user.type(screen.getByLabelText(/Tag for 2 items/), 'batch{Enter}');
    await waitFor(() => expect(state.entries.every((e) => e.tags.includes('batch'))).toBe(true));
  });

  it('declares dependencies between entries', async () => {
    const { user, state } = open({ entries: [file('a.txt'), file('b.txt')] });
    await user.click(await row('a.txt'));
    const insp = await screen.findByLabelText('Inspector', { selector: 'div' });
    await user.click(within(insp).getByRole('button', { name: 'Dependency target' }));
    await user.click(await screen.findByRole('option', { name: 'b.txt' }));
    await user.click(within(insp).getByRole('button', { name: 'Add dependency' }));
    await waitFor(() => expect(state.deps).toHaveLength(1));
    expect(state.deps[0].from_entry).toBe(state.entries[0].id);
  });
});

describe('search, sorting, filtering and views', () => {
  const seed = () => ({
    entries: [
      file('alpha.txt', { size: 50, tags: ['x'] }),
      file('beta.png', { size: 5000, mime: 'image/png' }),
      makeEntry({ workspace_id: 'w1', name: 'notes.md', kind: 'note', content: 'contains needle', size: 15 }),
      makeEntry({ workspace_id: 'w1', name: 'Docs', kind: 'folder' }),
    ],
  });

  it('searches names and note contents across the workspace', async () => {
    const { user } = open(seed());
    await itemsTable();
    await user.type(screen.getByLabelText('Search workspace'), 'needle');
    expect(await screen.findByText('1 result', { exact: false })).toBeInTheDocument();
    expect(await row('notes.md')).toBeInTheDocument();
    expect(screen.queryByText('alpha.txt')).toBeNull();
    await user.clear(screen.getByLabelText('Search workspace'));
    await user.type(screen.getByLabelText('Search workspace'), 'zzzz');
    expect(await screen.findByText('Nothing found')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Clear search' }));
    expect(await row('alpha.txt')).toBeInTheDocument();
  });

  it('filters by type and by tag', async () => {
    const { user } = open(seed());
    await itemsTable();
    await user.selectOptions(screen.getByLabelText('Filter by type'), 'note');
    expect(await row('notes.md')).toBeInTheDocument();
    expect(screen.queryByText('beta.png')).toBeNull();
    await user.selectOptions(screen.getByLabelText('Filter by type'), '');
    await user.click(await screen.findByRole('button', { name: /^x/ }));
    expect(await row('alpha.txt')).toBeInTheDocument();
    expect(screen.queryByText('beta.png')).toBeNull();
  });

  it('sorts by size and keeps folders first', async () => {
    const { user } = open(seed());
    await itemsTable();
    await user.selectOptions(screen.getByLabelText('Sort by'), 'size');
    let names = within(await itemsTable())
      .getAllByRole('row')
      .slice(1)
      .map((r) => r.querySelector('td')!.textContent);
    expect(names[0]).toContain('Docs');
    expect(names.indexOf(names.find((n) => n!.includes('alpha'))!)).toBeLessThan(
      names.indexOf(names.find((n) => n!.includes('beta'))!),
    );
    await user.click(within(await itemsTable()).getByRole('button', { name: 'Size' }));
    names = within(await itemsTable())
      .getAllByRole('row')
      .slice(1)
      .map((r) => r.querySelector('td')!.textContent);
    expect(names.indexOf(names.find((n) => n!.includes('beta'))!)).toBeLessThan(
      names.indexOf(names.find((n) => n!.includes('alpha'))!),
    );
  });

  it('switches between list and grid views', async () => {
    const { user } = open(seed());
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Grid view' }));
    const grid = await screen.findByRole('listbox', { name: 'Workspace contents' });
    expect(within(grid).getAllByRole('option')).toHaveLength(4);
    await user.click(within(grid).getByText('alpha.txt'));
    expect(await screen.findByText('Quarterly', { exact: false }).catch(() => null)).toBeNull();
    await user.click(screen.getByRole('button', { name: 'List view' }));
    expect(await itemsTable()).toBeInTheDocument();
  });
});

describe('source intelligence', () => {
  it('flags missing and changed sources and lets the user relink or accept', async () => {
    const { user, state } = open({ entries: [file('gone.txt'), file('edited.txt'), file('fine.txt')] });
    state.sources[state.entries[0].id] = { state: 'missing' };
    state.sources[state.entries[1].id] = { state: 'changed', reason: 'size changed' };
    expect(await screen.findByText(/2 source files are missing or changed/)).toBeInTheDocument();
    const gone = await row('gone.txt');
    expect(within(gone).getByText('Missing')).toBeInTheDocument();
    expect(within(await row('edited.txt')).getByText('Changed')).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: /Health/ }));
    await user.click(await screen.findByRole('button', { name: 'Accept change' }));
    await waitFor(() => expect(state.sources[state.entries[1].id]).toBeUndefined());
    state.picks.any = '/elsewhere/gone.txt';
    await user.click(await screen.findByRole('button', { name: 'Relink…' }));
    await waitFor(() => expect(state.entries[0].source_path).toBe('/elsewhere/gone.txt'));
    expect(await screen.findByText(/All 3 source files match/)).toBeInTheDocument();
  });

  it('asks for explicit confirmation before accepting a replacement with different content', async () => {
    const { user, state } = open({ entries: [file('gone.txt')] });
    state.sources[state.entries[0].id] = { state: 'missing' };
    state.picks.any = '/elsewhere/different.txt';
    await user.click(await screen.findByRole('tab', { name: /Health/ }));
    await user.click(await screen.findByRole('button', { name: 'Relink…' }));
    const dlg = await screen.findByRole('dialog', { name: 'Content differs' });
    expect(state.entries[0].source_path).toBe('/src/gone.txt');
    await user.click(within(dlg).getByRole('button', { name: 'Use this file anyway' }));
    await waitFor(() => expect(state.entries[0].source_path).toBe('/elsewhere/different.txt'));
  });

  it('lists duplicate groups and removes extra copies without touching originals', async () => {
    const { user, state } = open({
      entries: [file('one.txt', { sha256: 'same' }), file('two.txt', { sha256: 'same' }), file('other.txt')],
    });
    await user.click(await screen.findByRole('tab', { name: /Health/ }));
    expect(await screen.findByText(/2 identical files/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Remove extra copies…' }));
    expect(await screen.findByText(/Original files on disk are not touched/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Remove copies' }));
    await waitFor(() => expect(state.entries.map((e) => e.name).sort()).toEqual(['one.txt', 'other.txt']));
  });
});

describe('export', () => {
  it('exports a package through the save dialog and records the revision', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Export package' }));
    const dlg = await screen.findByRole('dialog', { name: 'Export package' });
    expect(within(dlg).getByText(/only read, never modified/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole('button', { name: 'Choose location and export' }));
    expect(await screen.findByText(/Exported 1 entry/)).toBeInTheDocument();
    expect(state.exported).toEqual([{ dest: '/out/Test.parcel', accept: false }]);
    expect(await screen.findByText('Revision 1')).toBeInTheDocument();
  });

  it('does nothing if the user cancels the save dialog', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    state.picks.save = null;
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Export package' }));
    await user.click(await screen.findByRole('button', { name: 'Choose location and export' }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Choose location and export' })).toBeEnabled(),
    );
    expect(state.exported).toHaveLength(0);
  });

  it('blocks export when sources are missing and explains exactly what is wrong', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    state.exportProblems = {
      missing: [{ entry_id: 'x', path: 'a.txt', source_path: '/src/a.txt', detail: 'source file not found' }],
      changed: [],
    };
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Export package' }));
    await user.click(await screen.findByRole('button', { name: 'Choose location and export' }));
    expect(await screen.findByText(/Nothing has been written/)).toBeInTheDocument();
    expect(screen.getByText('/src/a.txt')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Accept changes and export' })).toBeNull();
    expect(state.exported).toHaveLength(0);
  });

  it('offers to accept changed sources explicitly', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    state.exportProblems = {
      missing: [],
      changed: [
        { entry_id: 'x', path: 'a.txt', source_path: '/src/a.txt', detail: 'size changed from 1 to 2 bytes' },
      ],
    };
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Export package' }));
    await user.click(await screen.findByRole('button', { name: 'Choose location and export' }));
    expect(await screen.findByText(/size changed from 1 to 2 bytes/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Accept changes and export' }));
    await waitFor(() => expect(state.exported).toEqual([{ dest: '/out/Test.parcel', accept: true }]));
  });

  it('reports cancellation clearly', async () => {
    const { user } = open(
      { entries: [file('a.txt')] },
      {
        exportWorkspace: async () => {
          throw Object.assign(new Error('operation cancelled'), { code: 'cancelled' });
        },
      },
    );
    await itemsTable();
    await user.click(screen.getByRole('button', { name: 'Export package' }));
    await user.click(await screen.findByRole('button', { name: 'Choose location and export' }));
    expect(await screen.findByText(/Export cancelled. No package was written/)).toBeInTheDocument();
  });
});
