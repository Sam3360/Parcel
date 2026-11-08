import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from './test/helpers';
import type { Workspace } from './lib/types';

const ws = (id: string, name: string, extra: Partial<Workspace> = {}): Workspace => ({
  id,
  name,
  description: '',
  notes: '',
  metadata: {},
  package_id: 'p' + id,
  revision: 0,
  export_profile_id: null,
  template_id: null,
  last_export_path: null,
  created_at: '',
  modified_at: new Date().toISOString(),
  opened_at: null,
  deleted_at: null,
  ...extra,
});

describe('workspace management', () => {
  it('shows an empty state with a call to action', async () => {
    const { user } = renderApp({ route: { page: 'workspaces' } });
    expect(await screen.findByText('No workspaces yet')).toBeInTheDocument();
    await user.click(screen.getAllByRole('button', { name: /New workspace/ })[0]);
    expect(await screen.findByRole('dialog', { name: 'New workspace' })).toBeInTheDocument();
  });

  it('creates a workspace from a template with its folders', async () => {
    const { user, state } = renderApp({ route: { page: 'workspaces' } });
    await user.click((await screen.findAllByRole('button', { name: /New workspace/ }))[0]);
    await user.type(screen.getByLabelText('Name'), 'Study');
    await user.selectOptions(screen.getByLabelText('Start from'), 'tpl-research');
    expect(screen.getByText(/Creates 2 folders and 1 starter notes/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Create workspace' }));
    await waitFor(() => expect(state.entries.map((e) => e.name)).toEqual(['Sources', 'Data']));
    const tree = await screen.findByRole('tree', { name: 'Workspace structure' });
    expect(within(tree).getByText('Sources')).toBeInTheDocument();
  });

  it('rejects an empty name and surfaces backend validation errors', async () => {
    const { user } = renderApp({
      route: { page: 'workspaces' },
      api: {
        createWorkspace: async () => {
          throw new Error('workspace name must be 1-200 characters');
        },
      },
    });
    await user.click((await screen.findAllByRole('button', { name: /New workspace/ }))[0]);
    expect(screen.getByRole('button', { name: 'Create workspace' })).toBeDisabled();
    await user.type(screen.getByLabelText('Name'), 'x');
    await user.click(screen.getByRole('button', { name: 'Create workspace' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('1-200 characters');
  });

  it('renames, duplicates, trashes, restores and permanently deletes safely', async () => {
    const { user, state } = renderApp({
      route: { page: 'workspaces' },
      seed: { workspaces: [ws('a', 'Alpha'), ws('b', 'Beta')] },
    });
    await user.click(await screen.findByRole('button', { name: 'Actions for Alpha' }));
    await user.click(screen.getByRole('menuitem', { name: /Rename/ }));
    const input = screen.getByLabelText('Name');
    await user.clear(input);
    await user.type(input, 'Alpha 2{Enter}');
    await waitFor(() => expect(state.workspaces[0].name).toBe('Alpha 2'));

    await user.click(await screen.findByRole('button', { name: 'Actions for Alpha 2' }));
    await user.click(screen.getByRole('menuitem', { name: /Duplicate/ }));
    await user.click(screen.getByRole('button', { name: 'Duplicate' }));
    await waitFor(() => expect(state.workspaces).toHaveLength(3));

    await user.click(await screen.findByRole('button', { name: 'Actions for Beta' }));
    await user.click(screen.getByRole('menuitem', { name: /Move to trash/ }));
    expect(await screen.findByText(/original files are not touched/i)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Move to trash' }));
    await waitFor(() => expect(state.workspaces.find((w) => w.name === 'Beta')?.deleted_at).toBeTruthy());

    await user.click(screen.getByRole('button', { name: 'Trash' }));
    expect(await screen.findByRole('heading', { name: 'Trash' })).toBeInTheDocument();
    await user.click(await screen.findByRole('button', { name: 'Actions for Beta' }));
    await user.click(screen.getByRole('menuitem', { name: /Restore/ }));
    await waitFor(() => expect(state.workspaces.find((w) => w.name === 'Beta')?.deleted_at).toBeNull());

    await user.click(screen.getByRole('button', { name: 'Back to workspaces' }));
    await user.click(await screen.findByRole('button', { name: 'Actions for Beta' }));
    await user.click(screen.getByRole('menuitem', { name: /Move to trash/ }));
    await user.click(screen.getByRole('button', { name: 'Move to trash' }));
    await user.click(await screen.findByRole('button', { name: 'Trash' }));
    await user.click(await screen.findByRole('button', { name: 'Actions for Beta' }));
    await user.click(screen.getByRole('menuitem', { name: /Delete permanently/ }));
    expect(
      await screen.findByText(/Original files and exported packages are not deleted/),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Delete permanently' }));
    await waitFor(() => expect(state.workspaces.find((w) => w.name === 'Beta')).toBeUndefined());
  });
});
