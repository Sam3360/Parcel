import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { renderApp } from './test/helpers';

describe('templates and export profiles', () => {
  it('lists built-in templates and creates a workspace from one', async () => {
    const { user, state } = renderApp({ route: { page: 'templates' } });
    expect(await screen.findByText('Research collection')).toBeInTheDocument();
    expect(screen.getAllByText('Built-in').length).toBeGreaterThan(0);
    const card = screen.getByText('Research collection').closest('.card')!;
    await user.click(within(card as HTMLElement).getByRole('button', { name: 'Use template' }));
    expect(await screen.findByLabelText('Start from')).toHaveValue('tpl-research');
    await user.type(screen.getByLabelText('Name'), 'From template');
    await user.click(screen.getByRole('button', { name: 'Create workspace' }));
    await waitFor(() => expect(state.workspaces[0]?.template_id).toBe('tpl-research'));
    expect(state.entries.map((e) => e.name)).toContain('Sources');
  });

  it('creates and deletes a custom template', async () => {
    const { user, state } = renderApp({ route: { page: 'templates' } });
    await user.click(await screen.findByRole('button', { name: 'New template' }));
    await user.type(screen.getByLabelText('Name'), 'Mine');
    await user.type(screen.getByLabelText('Folders'), 'A/B{Enter}C');
    await user.click(screen.getByRole('button', { name: 'Save template' }));
    await waitFor(() =>
      expect(state.templates.find((t) => t.name === 'Mine')?.spec.folders).toEqual(['A/B', 'C']),
    );
    const card = (await screen.findByText('Mine')).closest('.card') as HTMLElement;
    await user.click(within(card).getByRole('button', { name: 'Delete' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(state.templates.find((t) => t.name === 'Mine')).toBeUndefined());
  });

  it('only offers a copy for built-in templates', async () => {
    renderApp({ route: { page: 'templates' } });
    await screen.findByText('Research collection');
    expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull();
    expect(screen.getAllByRole('button', { name: 'Save a copy…' }).length).toBeGreaterThan(0);
  });

  it('creates an export profile with exclusion patterns', async () => {
    const { user, state } = renderApp({ route: { page: 'templates' } });
    await user.click(await screen.findByRole('tab', { name: 'Export profiles' }));
    await user.click(await screen.findByRole('button', { name: 'New profile' }));
    await user.type(screen.getByLabelText('Name'), 'Lean');
    await user.click(screen.getByRole('checkbox', { name: 'Compress files' }));
    await user.type(screen.getByLabelText('Exclude files matching'), '*.tmp{Enter}~$*');
    await user.click(screen.getByRole('button', { name: 'Save profile' }));
    await waitFor(() => {
      const p = state.profiles.find((x) => x.name === 'Lean');
      expect(p?.spec.compress).toBe(false);
      expect(p?.spec.exclude_patterns).toEqual(['*.tmp', '~$*']);
    });
  });
});
