import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from './test/helpers';
import { makeEntry, type MockState } from './test/mockApi';
import type { Workspace } from './lib/types';

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
const file = (name: string, extra: Partial<ReturnType<typeof makeEntry>> = {}) =>
  makeEntry({
    workspace_id: 'w1',
    name,
    kind: 'file',
    source_path: `/src/${name}`,
    size: 100,
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
const table = () => screen.findByRole('table', { name: 'Workspace contents' });
const row = async (name: string) => (await within(await table()).findByText(name)).closest('tr')!;
const removeSelected = async (user: ReturnType<typeof renderApp>['user'], name: string) => {
  await user.click(await row(name));
  await user.keyboard('{Delete}');
  await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Remove' }));
};

describe('undo of "remove from workspace"', () => {
  it('offers Undo right after a removal and restores the item', async () => {
    const { user, state } = open({ entries: [file('a.txt'), file('b.txt')] });
    await removeSelected(user, 'a.txt');
    await waitFor(() => expect(state.entries.map((e) => e.name)).toEqual(['b.txt']));
    expect(await screen.findByText('Removed 1 item from the workspace')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Undo' }));
    expect(await screen.findByText('Restored 1 item')).toBeInTheDocument();
    expect(await row('a.txt')).toBeInTheDocument();
    expect(state.entries.map((e) => e.name).sort()).toEqual(['a.txt', 'b.txt']);
  });

  it('Ctrl+Z undoes the last removal, and the toolbar button tracks how many are pending', async () => {
    const { user, state } = open({ entries: [file('a.txt'), file('b.txt'), file('c.txt')] });
    expect(screen.queryByRole('button', { name: /Undo remove/ })).toBeNull();
    await removeSelected(user, 'a.txt');
    expect(await screen.findByRole('button', { name: 'Undo remove' })).toBeInTheDocument();
    await removeSelected(user, 'b.txt');
    expect(await screen.findByRole('button', { name: 'Undo remove (2)' })).toBeInTheDocument();
    await user.keyboard('{Control>}z{/Control}');
    await waitFor(() => expect(state.entries.map((e) => e.name).sort()).toEqual(['b.txt', 'c.txt']));
    await user.click(await screen.findByRole('button', { name: 'Undo remove' }));
    await waitFor(() => expect(state.entries.map((e) => e.name).sort()).toEqual(['a.txt', 'b.txt', 'c.txt']));
    await waitFor(() => expect(screen.queryByRole('button', { name: /Undo remove/ })).toBeNull());
    await user.keyboard('{Control>}z{/Control}');
    expect(await screen.findByText(/nothing to undo/i)).toBeInTheDocument();
  });

  it('does not steal Ctrl+Z from text fields', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    await removeSelected(user, 'a.txt');
    await screen.findByRole('button', { name: 'Undo remove' });
    await user.click(screen.getByLabelText('Search workspace'));
    await user.keyboard('{Control>}z{/Control}');
    expect(state.entries).toHaveLength(0);
    expect(screen.getByRole('button', { name: 'Undo remove' })).toBeInTheDocument();
  });

  it('restores a removed folder together with everything inside it', async () => {
    const dir = makeEntry({ workspace_id: 'w1', name: 'Dir', kind: 'folder' });
    const { user, state } = open({ entries: [dir, file('in.txt', { parent_id: dir.id })] });
    await removeSelected(user, 'Dir');
    await waitFor(() => expect(state.entries).toHaveLength(0));
    await user.keyboard('{Control>}z{/Control}');
    await waitFor(() => expect(state.entries.map((e) => e.name).sort()).toEqual(['Dir', 'in.txt']));
    expect(state.entries.find((e) => e.name === 'in.txt')!.parent_id).toBe(dir.id);
  });

  it('says so when a restored item had to be renamed or moved to the top level', async () => {
    const dir = makeEntry({ workspace_id: 'w1', name: 'P', kind: 'folder' });
    const { user, state } = open({ entries: [dir, file('x.txt', { parent_id: dir.id }), file('y.txt')] });
    await user.dblClick(await row('P')); // open the folder, then remove x.txt inside it
    await removeSelected(user, 'x.txt');
    await waitFor(() => expect(state.entries.some((e) => e.name === 'x.txt')).toBe(false));
    state.entries = state.entries.filter((e) => e.name !== 'P'); // its folder is gone
    state.entries.push(file('x.txt')); // and the name is taken at the top level
    await user.keyboard('{Control>}z{/Control}');
    expect(
      await screen.findByText(
        /Restored 1 item \(1 renamed because the name was taken; 1 placed at the top level because their folder is gone\)/,
      ),
    ).toBeInTheDocument();
    expect(state.entries.map((e) => e.name).sort()).toEqual(['x (1).txt', 'x.txt', 'y.txt']);
  });

  it('shows an error and keeps working when undo fails', async () => {
    const { user } = open(
      { entries: [file('a.txt')] },
      {
        undoRemove: async () => {
          throw new Error('restore the workspace from the trash before undoing');
        },
      },
    );
    await removeSelected(user, 'a.txt');
    await user.click(await screen.findByRole('button', { name: 'Undo' }));
    expect(
      await screen.findByText(/Could not undo: restore the workspace from the trash/),
    ).toBeInTheDocument();
  });

  it('leaving the workspace makes pending removals final', async () => {
    const { user, state } = open({ entries: [file('a.txt')] });
    await removeSelected(user, 'a.txt');
    await screen.findByRole('button', { name: 'Undo remove' });
    expect(state.finalized).not.toContain('w1');
    await user.click(screen.getByRole('button', { name: 'Back to workspaces' }));
    await waitFor(() => expect(state.finalized).toContain('w1'));
    expect(state.removals.w1).toBeUndefined();
  });

  it('can undo removing duplicate copies from the Health tab', async () => {
    const { user, state } = open({
      entries: [file('one.txt', { sha256: 'same' }), file('two.txt', { sha256: 'same' })],
    });
    await user.click(await screen.findByRole('tab', { name: /Health/ }));
    await user.click(await screen.findByRole('button', { name: 'Remove extra copies…' }));
    await user.click(await screen.findByRole('button', { name: 'Remove copies' }));
    await waitFor(() => expect(state.entries.map((e) => e.name)).toEqual(['one.txt']));
    await user.click(await screen.findByRole('button', { name: 'Undo' }));
    await waitFor(() => expect(state.entries.map((e) => e.name).sort()).toEqual(['one.txt', 'two.txt']));
  });
});

describe('the removal confirmation', () => {
  it('reassures and mentions undo for files you collected yourself', async () => {
    const { user } = open({ entries: [file('a.txt')] });
    await user.click(await row('a.txt'));
    await user.keyboard('{Delete}');
    const dlg = await screen.findByRole('dialog');
    expect(within(dlg).getByText(/Your original files are not deleted or changed/)).toBeInTheDocument();
    expect(within(dlg).getByText(/You can undo this \(Ctrl\+Z\)/)).toBeInTheDocument();
    expect(within(dlg).queryByText(/imported package/)).toBeNull();
  });

  it('is explicit that imported files are Parcel-stored copies, counting files inside removed folders', async () => {
    const dir = makeEntry({ workspace_id: 'w1', name: 'Dir', kind: 'folder' });
    const imported = (n: string, parent: string | null) => file(n, { parent_id: parent, origin: 'imported' });
    const { user } = open({
      entries: [
        dir,
        imported('i1.txt', dir.id),
        imported('i2.txt', dir.id),
        file('mine.txt', { parent_id: dir.id }),
      ],
    });
    await user.click(await row('Dir'));
    await user.keyboard('{Delete}');
    const dlg = await screen.findByRole('dialog');
    expect(
      within(dlg).getByText(/2 files came from an imported package, so Parcel keeps its own copy/),
    ).toBeInTheDocument();
    expect(within(dlg).getByText(/the package you imported from is not changed/)).toBeInTheDocument();
    expect(within(dlg).getByText(/Until then you can undo/)).toBeInTheDocument();
  });

  it('skips the dialog when the preference is off, and Undo is still offered', async () => {
    const { user } = open({ entries: [file('a.txt')], prefs: { confirm_remove: 'false' } });
    await user.click(await row('a.txt'));
    await user.keyboard('{Delete}');
    expect(await screen.findByRole('button', { name: 'Undo' })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('Settings → Storage', () => {
  const usage = {
    files: 12,
    bytes: 3 * 1024 * 1024,
    unused_files: 0,
    unused_bytes: 0,
    unused_examples: [] as string[],
    orphan_folders: 0,
  };
  const open_ = (report = usage, api = {}) =>
    renderApp({ route: { page: 'settings' }, seed: { storage: report }, api });

  it('summarises Parcel-stored files and disables cleanup when nothing is unused', async () => {
    open_();
    const dl = await screen.findByLabelText('Storage summary');
    expect(within(dl).getByText(/12 files \(3\.0 MB\)/)).toBeInTheDocument();
    expect(within(dl).getByText(/0 files \(0 B\)/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove unused files…' })).toBeDisabled();
    expect(screen.getByText(/Files you collect from your computer stay where they are/)).toBeInTheDocument();
  });

  it('shows what would be removed, asks for confirmation, then cleans up and refreshes', async () => {
    const { user, state } = open_({
      ...usage,
      unused_files: 2,
      unused_bytes: 2048,
      unused_examples: ['abc/Leftover/a.bin', 'abc/b.bin'],
      orphan_folders: 1,
    });
    expect(await screen.findByText('abc/Leftover/a.bin')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Remove unused files…' }));
    const dlg = await screen.findByRole('dialog');
    expect(within(dlg).getByText(/deletes 2 files \(2\.0 KB\) and 1 leftover folder/)).toBeInTheDocument();
    expect(within(dlg).getByText(/not touched/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole('button', { name: 'Cancel' }));
    expect(state.storageRemoved).toBe(0);
    await user.click(screen.getByRole('button', { name: 'Remove unused files…' }));
    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Remove unused files' }),
    );
    await waitFor(() => expect(state.storageRemoved).toBe(1));
    expect(await screen.findByText('Freed 2.0 KB')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', { name: 'Remove unused files…' })).toBeDisabled());
  });

  it('degrades gracefully if storage information is unavailable', async () => {
    open_(usage, {
      storageReport: async () => {
        throw new Error('nope');
      },
    });
    expect(await screen.findByText('Storage information is not available.')).toBeInTheDocument();
  });

  it('reports a failed cleanup without crashing', async () => {
    const { user } = open_(
      { ...usage, unused_files: 1, unused_bytes: 10, unused_examples: ['x/y.bin'] },
      {
        removeUnusedStorage: async () => {
          throw new Error('disk is read-only');
        },
      },
    );
    await user.click(await screen.findByRole('button', { name: 'Remove unused files…' }));
    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Remove unused files' }),
    );
    expect(await screen.findByText(/Could not clean up storage: disk is read-only/)).toBeInTheDocument();
  });
});

describe('shortcuts', () => {
  it('lists Ctrl+Z', async () => {
    const { user } = renderApp();
    await user.keyboard('?');
    const dlg = await screen.findByRole('dialog', { name: 'Keyboard shortcuts' });
    expect(within(dlg).getByText('Ctrl+Z')).toBeInTheDocument();
    expect(within(dlg).getByText(/Undo the last removal/)).toBeInTheDocument();
    void act;
  });
});
