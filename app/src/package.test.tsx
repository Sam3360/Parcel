import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from './test/helpers';
import { samplePackage } from './test/mockApi';

const P = '/pk/Demo.parcel';
const openPkg = (extra: Parameters<typeof renderApp>[0] = {}) =>
  renderApp({
    route: { page: 'open', path: P },
    ...extra,
    seed: { packages: { [P]: samplePackage(P) }, ...(extra.seed ?? {}) },
  });

describe('package explorer', () => {
  it('shows an empty state and recent packages on the landing page', async () => {
    const { user, state } = renderApp({ route: { page: 'open' } });
    expect(await screen.findByText('Nothing opened yet')).toBeInTheDocument();
    state.packages[P] = samplePackage(P);
    state.picks.pkg = P;
    await user.click(screen.getByRole('button', { name: 'Open package…' }));
    expect(await screen.findByRole('button', { name: 'Verify integrity' })).toBeInTheDocument();
    expect((await screen.findAllByText('Demo package')).length).toBeGreaterThan(0);
  });

  it('browses folders, shows details, and works without any workspace', async () => {
    const { user } = openPkg();
    const table = await screen.findByRole('table', { name: 'Package entries' });
    expect(within(table).getByText('Docs')).toBeInTheDocument();
    expect(within(table).getByText('image.png')).toBeInTheDocument();
    await user.dblClick(within(table).getByText('Docs'));
    const crumbs = await screen.findByText('report.txt');
    expect(crumbs).toBeInTheDocument();
    expect(screen.queryByText('image.png')).toBeNull();
    await user.click(screen.getByText('report.txt'));
    const insp = screen.getByRole('complementary', { name: 'Details' });
    expect(await within(insp).findByText('The report')).toBeInTheDocument();
    expect(await within(insp).findByText('package entry a')).toBeInTheDocument();
    expect(within(insp).getByText('a'.repeat(64))).toBeInTheDocument();
  });

  it('searches package contents', async () => {
    const { user } = openPkg();
    await screen.findByRole('table', { name: 'Package entries' });
    await user.type(screen.getByLabelText('Search package'), 'report');
    const table = await screen.findByRole('table', { name: 'Package entries' });
    expect(within(table).getByText('report.txt')).toBeInTheDocument();
    expect(within(table).queryByText('image.png')).toBeNull();
    await user.clear(screen.getByLabelText('Search package'));
    await user.type(screen.getByLabelText('Search package'), 'nomatch');
    expect(await screen.findByText('No matches')).toBeInTheDocument();
  });

  it('shows notes and stored references without contacting them', async () => {
    const { user } = openPkg();
    await user.click(await screen.findByRole('tab', { name: 'Notes' }));
    expect(await screen.findByRole('heading', { name: 'Package notes' })).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: /References/ }));
    expect(await screen.findByText('https://doc.rust-lang.org')).toBeInTheDocument();
  });

  it('verifies integrity and reports success', async () => {
    const { user } = openPkg();
    await user.click(await screen.findByRole('button', { name: 'Verify integrity' }));
    expect(await screen.findByText(/All 2 files match their checksums/)).toBeInTheDocument();
    expect(screen.getAllByText('Verified').length).toBeGreaterThan(0);
  });

  it('reports corrupted entries clearly and marks them in the list', async () => {
    const { user } = openPkg({
      seed: {
        verify: {
          [P]: {
            ok: false,
            checked: 2,
            failures: [{ id: 'a', path: 'Docs/report.txt', status: 'hash_mismatch' }],
            metadata_failures: [],
            unlisted: [],
            warnings: [],
          },
        },
      },
    });
    await user.click(await screen.findByRole('button', { name: 'Verify integrity' }));
    expect(await screen.findByText(/1 problem found/)).toBeInTheDocument();
    const failures = screen.getByRole('table', { name: 'Integrity failures' });
    expect(within(failures).getByText('Docs/report.txt')).toBeInTheDocument();
    expect(within(failures).getByText('hash mismatch')).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: /Contents/ }));
    await user.dblClick(
      within(await screen.findByRole('table', { name: 'Package entries' })).getByText('Docs'),
    );
    expect(await screen.findByText('Corrupted')).toBeInTheDocument();
  });

  it('extracts the entire package and selected items, choosing a conflict policy', async () => {
    const { user, state } = openPkg();
    state.picks.dir = '/dest';
    await user.click(await screen.findByRole('button', { name: 'Extract all…' }));
    const dlg = await screen.findByRole('dialog');
    expect(within(dlg).getByText(/never overwritten/)).toBeInTheDocument();
    await user.click(within(dlg).getByRole('radio', { name: /Keep both/ }));
    await user.click(within(dlg).getByRole('button', { name: 'Choose folder and extract' }));
    await waitFor(() =>
      expect(state.extracted).toEqual([{ path: P, dest: '/dest', ids: null, policy: 'rename' }]),
    );
    expect(screen.getByRole('button', { name: 'Extract selected…' })).toBeDisabled();
    await user.click(screen.getByText('image.png'));
    await user.click(screen.getByRole('button', { name: 'Extract selected…' }));
    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Choose folder and extract' }),
    );
    await waitFor(() =>
      expect(state.extracted[1]).toEqual({ path: P, dest: '/dest', ids: ['b'], policy: 'fail' }),
    );
  });

  it('explains that nothing was written when extraction hits an existing file', async () => {
    const { user, state } = openPkg({
      api: {
        extractPackage: async () => {
          throw Object.assign(new Error('destination already exists: /dest/a.txt'), {
            code: 'already_exists',
          });
        },
      },
    });
    state.picks.dir = '/dest';
    await user.click(await screen.findByRole('button', { name: 'Extract all…' }));
    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Choose folder and extract' }),
    );
    expect(await screen.findByText(/Nothing was written/)).toBeInTheDocument();
  });

  it('imports a package as an editable workspace after confirming', async () => {
    const { user, state } = openPkg();
    await user.click(await screen.findByRole('button', { name: 'Import as workspace' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Import' }));
    await waitFor(() => expect(state.workspaces.map((w) => w.name)).toEqual(['Demo package']));
    expect(await screen.findByRole('toolbar', { name: 'Workspace tools' })).toBeInTheDocument();
  });

  it('compares two package versions and states what the comparison is based on', async () => {
    const { user, state } = openPkg();
    state.picks.pkg = '/pk/Older.parcel';
    state.packages['/pk/Older.parcel'] = samplePackage('/pk/Older.parcel');
    await user.click(await screen.findByRole('button', { name: 'Compare…' }));
    expect(await screen.findByText(/Based on checksums and metadata only/)).toBeInTheDocument();
    expect(screen.getByText('new.txt')).toBeInTheDocument();
    expect(screen.getByText('a.txt')).toBeInTheDocument();
    expect(screen.getByText('size 1 → 2 bytes')).toBeInTheDocument();
    expect(screen.getByText(/1 entry unchanged/)).toBeInTheDocument();
  });

  it('refuses unsafe or invalid packages with a calm explanation', async () => {
    renderApp({ route: { page: 'open', path: '/bad/evil.parcel' } });
    expect(await screen.findByText('This package could not be opened')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent(/manifest.json is missing/);
    expect(screen.getByText(/Nothing was extracted or run/)).toBeInTheDocument();
  });

  it('opens a link only after confirmation', async () => {
    let opened = '';
    const { user } = openPkg({
      api: {
        openExternal: async (u: string) => {
          opened = u;
        },
      },
    });
    await user.click(await screen.findByText('Site'));
    await user.click(await screen.findByRole('button', { name: 'Open link' }));
    expect(opened).toBe('');
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Open' }));
    await waitFor(() => expect(opened).toBe('https://example.com'));
  });
});
