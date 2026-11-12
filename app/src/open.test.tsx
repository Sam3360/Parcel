import { act, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from './test/helpers';
import { samplePackage } from './test/mockApi';

const P = '/pk/Demo.parcel';
const withPackage = (extra: Parameters<typeof renderApp>[0] = {}) =>
  renderApp({ ...extra, seed: { packages: { [P]: samplePackage(P) }, ...(extra.seed ?? {}) } });

describe('opening packages from outside the app', () => {
  it('opens a package passed at launch (double-click / Open with)', async () => {
    const { state } = withPackage({ seed: { launch: [P] } });
    expect(await screen.findByRole('button', { name: 'Verify integrity' })).toBeInTheDocument();
    expect(screen.getAllByText('Demo package').length).toBeGreaterThan(0);
    expect(state.launch).toEqual([]); // the queue is drained exactly once
  });

  it('opens a package forwarded by a second instance while the app is running', async () => {
    const { state } = withPackage();
    expect(await screen.findByText('No workspaces yet')).toBeInTheDocument();
    state.launch.push(P);
    act(() => state.openHandlers.forEach((h) => h()));
    expect(await screen.findByRole('button', { name: 'Verify integrity' })).toBeInTheDocument();
  });

  it('opens only the first of several launch packages and says so', async () => {
    withPackage({ seed: { launch: [P, '/pk/Second.parcel'] } });
    expect(await screen.findByText(/Opened the first of 2 packages/)).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Verify integrity' })).toBeInTheDocument();
  });

  it('reports a package that cannot be opened instead of crashing', async () => {
    renderApp({ seed: { launch: ['/gone/Missing.parcel'] } });
    expect(await screen.findByText('This package could not be opened')).toBeInTheDocument();
  });

  it('a dropped .parcel opens from Home, Templates, Settings and the package page', async () => {
    const { state, user } = withPackage();
    await screen.findByText('No workspaces yet');
    act(() => state.dropHandlers.forEach((h) => h([P])));
    expect(await screen.findByRole('button', { name: 'Verify integrity' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Close package' }));
    await user.click(screen.getByRole('button', { name: 'Templates' }));
    await screen.findByRole('heading', { name: 'Templates' });
    act(() => state.dropHandlers.forEach((h) => h([P])));
    expect(await screen.findByRole('button', { name: 'Verify integrity' })).toBeInTheDocument();
  });

  it('dropping ordinary files outside a workspace explains what to do and changes nothing', async () => {
    const { state } = withPackage();
    await screen.findByText('No workspaces yet');
    act(() => state.dropHandlers.forEach((h) => h(['/x/photo.png'])));
    expect(await screen.findByText(/open a workspace and drop them there/i)).toBeInTheDocument();
    expect(state.workspaces).toHaveLength(0);
    expect(state.entries).toHaveLength(0);
  });

  it('unsubscribes its handlers when unmounted', async () => {
    const { state, unmount } = withPackage();
    await screen.findByText('No workspaces yet');
    expect(state.dropHandlers.size).toBeGreaterThan(0);
    expect(state.openHandlers.size).toBeGreaterThan(0);
    unmount();
    await waitFor(() => expect(state.dropHandlers.size + state.openHandlers.size).toBe(0));
  });
});

describe('show in folder', () => {
  it('reveals a collected file from the inspector using its entry id, never a path from the UI', async () => {
    const { makeEntry } = await import('./test/mockApi');
    const e = makeEntry({
      workspace_id: 'w1',
      name: 'a.txt',
      kind: 'file',
      source_path: '/src/a.txt',
      size: 5,
      sha256: 'h',
    });
    const ws = {
      id: 'w1',
      name: 'P',
      description: '',
      notes: '',
      metadata: {},
      package_id: 'p',
      revision: 0,
      export_profile_id: null,
      template_id: null,
      last_export_path: null,
      created_at: '',
      modified_at: new Date().toISOString(),
      opened_at: null,
      deleted_at: null,
    };
    const { state, user } = renderApp({
      route: { page: 'workspaces', workspaceId: 'w1' },
      seed: { workspaces: [ws], entries: [e] },
    });
    await user.click(await screen.findByText('a.txt'));
    await user.click(await screen.findByRole('button', { name: 'Show in folder' }));
    await waitFor(() => expect(state.revealed).toEqual([{ kind: 'source', value: e.id }]));
  });

  it('offers to show an exported package in its folder', async () => {
    const { makeEntry } = await import('./test/mockApi');
    const e = makeEntry({
      workspace_id: 'w1',
      name: 'a.txt',
      kind: 'file',
      source_path: '/src/a.txt',
      size: 5,
      sha256: 'h',
    });
    const ws = {
      id: 'w1',
      name: 'P',
      description: '',
      notes: '',
      metadata: {},
      package_id: 'p',
      revision: 0,
      export_profile_id: null,
      template_id: null,
      last_export_path: null,
      created_at: '',
      modified_at: new Date().toISOString(),
      opened_at: null,
      deleted_at: null,
    };
    const { state, user } = renderApp({
      route: { page: 'workspaces', workspaceId: 'w1' },
      seed: { workspaces: [ws], entries: [e] },
    });
    await screen.findByText('a.txt');
    await user.click(screen.getByRole('button', { name: 'Export package' }));
    await user.click(await screen.findByRole('button', { name: 'Choose location and export' }));
    await user.click(await screen.findByRole('button', { name: 'Show in folder' }));
    await waitFor(() => expect(state.revealed).toEqual([{ kind: 'output', value: '/out/Test.parcel' }]));
  });

  it('offers to show the extraction folder and the opened package', async () => {
    const { state, user } = withPackage({ route: { page: 'open', path: P } });
    state.picks.dir = '/dest';
    await user.click(await screen.findByRole('button', { name: 'Show package in folder' }));
    await user.click(screen.getByRole('button', { name: 'Extract all…' }));
    await user.click(await screen.findByRole('button', { name: 'Choose folder and extract' }));
    await user.click(await screen.findByRole('button', { name: 'Show in folder' }));
    await waitFor(() =>
      expect(state.revealed).toEqual([
        { kind: 'output', value: P },
        { kind: 'output', value: '/dest' },
      ]),
    );
  });

  it('shows an error toast when revealing fails', async () => {
    const { user } = withPackage({
      route: { page: 'open', path: P },
      api: {
        revealOutput: async () => {
          throw new Error('that location no longer exists');
        },
      },
    });
    await user.click(await screen.findByRole('button', { name: 'Show package in folder' }));
    expect(await screen.findByText(/that location no longer exists/)).toBeInTheDocument();
  });
});
