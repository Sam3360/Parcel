import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from './test/helpers';
import { makeEntry } from './test/mockApi';

describe('navigation and shell', () => {
  it('shows the six main sections and navigates between them', async () => {
    const { user } = renderApp();
    const nav = screen.getByRole('navigation', { name: 'Main' });
    for (const n of ['Home', 'Workspaces', 'Open Package', 'Templates', 'Activity', 'Settings'])
      expect(within(nav).getByRole('button', { name: n })).toBeInTheDocument();
    expect(within(nav).getByRole('button', { name: 'Home' })).toHaveAttribute('aria-current', 'page');
    await user.click(within(nav).getByRole('button', { name: 'Workspaces' }));
    expect(await screen.findByRole('heading', { name: 'Workspaces' })).toBeInTheDocument();
    expect(within(nav).getByRole('button', { name: 'Workspaces' })).toHaveAttribute('aria-current', 'page');
    await user.click(within(nav).getByRole('button', { name: 'Templates' }));
    expect(await screen.findByRole('heading', { name: 'Templates' })).toBeInTheDocument();
    await user.click(within(nav).getByRole('button', { name: 'Activity' }));
    expect(await screen.findByRole('heading', { name: 'Activity' })).toBeInTheDocument();
    await user.click(within(nav).getByRole('button', { name: 'Settings' }));
    expect(await screen.findByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    await user.click(within(nav).getByRole('button', { name: 'Open Package' }));
    expect(await screen.findByRole('heading', { name: 'Open Package' })).toBeInTheDocument();
  });

  it('supports keyboard navigation shortcuts and the shortcuts dialog', async () => {
    const { user } = renderApp();
    await user.keyboard('{Control>}2{/Control}');
    expect(await screen.findByRole('heading', { name: 'Workspaces' })).toBeInTheDocument();
    await user.keyboard('{Control>}5{/Control}');
    expect(await screen.findByRole('heading', { name: 'Activity' })).toBeInTheDocument();
    await user.keyboard('?');
    const dlg = await screen.findByRole('dialog', { name: 'Keyboard shortcuts' });
    expect(within(dlg).getByText('Ctrl+Shift+N')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('states the offline / no-telemetry promise', () => {
    renderApp();
    expect(screen.getByText(/no telemetry/i)).toBeInTheDocument();
  });
});

describe('home screen', () => {
  it('shows onboarding empty states when nothing exists', async () => {
    renderApp();
    expect(await screen.findByText('No workspaces yet')).toBeInTheDocument();
    expect(screen.getByText('No packages opened yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Open package/ })).toBeInTheDocument();
  });

  it('creates a workspace from the quick-create box and opens the editor', async () => {
    const { user, state } = renderApp();
    const create = screen.getByRole('button', { name: 'Create' });
    expect(create).toBeDisabled();
    await user.type(screen.getByLabelText('New workspace name'), 'Thesis');
    await user.click(create);
    await user.click(await screen.findByRole('button', { name: 'Create workspace' }));
    await waitFor(() => expect(state.workspaces).toHaveLength(1));
    expect(state.workspaces[0].name).toBe('Thesis');
    expect(await screen.findByRole('toolbar', { name: 'Workspace tools' })).toBeInTheDocument();
    expect(screen.getByText('Add your first items')).toBeInTheDocument();
  });

  it('lists recent workspaces and packages', async () => {
    const stamp = new Date().toISOString();
    renderApp({
      seed: {
        recentPackages: [{ path: '/p/a.parcel', name: 'Alpha pack', opened_at: stamp, size: 2048 }],
        workspaces: [
          {
            id: 'w1',
            name: 'Recent one',
            description: '',
            notes: '',
            metadata: {},
            package_id: 'p',
            revision: 0,
            export_profile_id: null,
            template_id: null,
            last_export_path: null,
            created_at: '',
            modified_at: stamp,
            opened_at: stamp,
            deleted_at: null,
          },
        ],
      },
    });
    expect(await screen.findByText('Alpha pack')).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: /Recent one/ })).toBeInTheDocument();
  });

  it('shows an error toast when loading fails but stays usable', async () => {
    renderApp({
      api: {
        recentWorkspaces: async () => {
          throw new Error('disk on fire');
        },
      },
    });
    expect(await screen.findByText(/disk on fire/)).toBeInTheDocument();
    expect(screen.getByLabelText('New workspace name')).toBeInTheDocument();
  });
});

describe('settings', () => {
  it('changes theme, persists it, and applies it to the document', async () => {
    const { user, state } = renderApp();
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    await user.selectOptions(await screen.findByLabelText('Theme'), 'dark');
    expect(state.prefs.theme).toBe('dark');
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('dark'));
    await user.selectOptions(screen.getByLabelText('Theme'), 'light');
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe('light'));
  });

  it('saves security limits and states the privacy position', async () => {
    const { user, state } = renderApp();
    await user.click(screen.getByRole('button', { name: 'Settings' }));
    const ratio = await screen.findByLabelText('Maximum compression ratio');
    await user.clear(ratio);
    await user.type(ratio, '50');
    await user.click(screen.getByRole('button', { name: 'Save limits' }));
    await waitFor(() => expect(JSON.parse(state.prefs.limits).max_compression_ratio).toBe(50));
    expect(screen.getByText(/no telemetry, no analytics/i)).toBeInTheDocument();
    await user.click(screen.getByRole('checkbox', { name: /Reduce motion/ }));
    expect(state.prefs.reduce_motion).toBe('true');
  });
});

describe('activity', () => {
  it('shows history, filters it, and clears it after confirmation', async () => {
    const { user, state } = renderApp();
    await state.workspaces;
    await user.click(screen.getByRole('button', { name: 'Activity' }));
    expect(await screen.findByText('Nothing here yet')).toBeInTheDocument();
    state.activity.push(
      {
        id: 1,
        ts: new Date().toISOString(),
        workspace_id: null,
        kind: 'package.exported',
        message: 'Exported revision 1',
        detail: null,
      },
      {
        id: 2,
        ts: new Date().toISOString(),
        workspace_id: null,
        kind: 'workspace.created',
        message: 'Created workspace “X”',
        detail: null,
      },
    );
    await user.click(screen.getByRole('button', { name: 'Home' }));
    await user.click(screen.getByRole('button', { name: 'Activity' }));
    expect(await screen.findByText('Exported revision 1')).toBeInTheDocument();
    await user.type(screen.getByLabelText('Filter activity'), 'exported');
    expect(screen.queryByText(/Created workspace/)).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Clear history' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Clear' }));
    await waitFor(() => expect(state.activity).toHaveLength(0));
  });
});

void makeEntry;
