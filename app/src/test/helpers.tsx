import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { App } from '../App';
import { AppProvider, type Route } from '../hooks/app';
import type { Api } from '../lib/api';
import { createMockApi, type MockState } from './mockApi';

export function renderApp(opts: { route?: Route; api?: Partial<Api>; seed?: Partial<MockState> } = {}) {
  const { api, state } = createMockApi(opts.api, opts.seed);
  const user = userEvent.setup();
  const utils = render(
    <AppProvider api={api} initialRoute={opts.route}>
      <App />
    </AppProvider>,
  );
  return { user, api, state, ...utils };
}
export function renderWith(
  ui: ReactElement,
  opts: { api?: Partial<Api>; seed?: Partial<MockState>; route?: Route } = {},
) {
  const { api, state } = createMockApi(opts.api, opts.seed);
  const user = userEvent.setup();
  return {
    user,
    api,
    state,
    ...render(
      <AppProvider api={api} initialRoute={opts.route}>
        {ui}
      </AppProvider>,
    ),
  };
}
