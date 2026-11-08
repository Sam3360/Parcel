import { useEffect, useState } from 'react';
import { useApp, type Route } from './hooks/app';
import { Icon, type IconName } from './components/Icon';
import { ErrorBoundary } from './components/ErrorBoundary';
import { isTyping } from './lib/shortcuts';
import { HomePage } from './pages/Home';
import { WorkspacesPage } from './pages/Workspaces';
import { OpenPackagePage } from './pages/OpenPackage';
import { TemplatesPage } from './pages/Templates';
import { ActivityPage } from './pages/Activity';
import { SettingsPage } from './pages/Settings';

const NAV: { page: Route['page']; label: string; icon: IconName }[] = [
  { page: 'home', label: 'Home', icon: 'home' },
  { page: 'workspaces', label: 'Workspaces', icon: 'layers' },
  { page: 'open', label: 'Open Package', icon: 'package' },
  { page: 'templates', label: 'Templates', icon: 'template' },
  { page: 'activity', label: 'Activity', icon: 'activity' },
  { page: 'settings', label: 'Settings', icon: 'settings' },
];

export function App() {
  const { route, navigate, api, showShortcuts } = useApp();
  const [version, setVersion] = useState('');
  const [resetKey, setResetKey] = useState(0);
  useEffect(() => {
    api
      .appInfo()
      .then((i) => setVersion(i.version))
      .catch(() => {});
  }, [api]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.shiftKey && !e.altKey && /^[1-6]$/.test(e.key)) {
        e.preventDefault();
        navigate({ page: NAV[Number(e.key) - 1].page } as Route);
      } else if (e.ctrlKey && e.key === 'o') {
        e.preventDefault();
        navigate({ page: 'open' });
      } else if (e.ctrlKey && e.key === ',') {
        e.preventDefault();
        navigate({ page: 'settings' });
      } else if (e.key === '?' && !isTyping(e.target)) {
        e.preventDefault();
        showShortcuts();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [navigate, showShortcuts]);

  return (
    <div className="shell">
      <nav className="nav" aria-label="Main">
        <div className="brand">
          <Icon name="box" size={22} /> Parcel
        </div>
        {NAV.map((n) => (
          <button
            key={n.page}
            className="nav-item"
            aria-current={route.page === n.page ? 'page' : undefined}
            onClick={() => navigate({ page: n.page } as Route)}
          >
            <Icon name={n.icon} /> {n.label}
          </button>
        ))}
        <div className="nav-spacer" />
        <button className="nav-item" onClick={showShortcuts}>
          <Icon name="keyboard" /> Shortcuts
        </button>
        <div className="nav-foot">
          Works offline. No account,
          <br />
          no telemetry.
          {version && (
            <>
              <br />v{version}
            </>
          )}
        </div>
      </nav>
      <main className="main" id="main">
        <ErrorBoundary key={`${route.page}-${resetKey}`} onReset={() => setResetKey((k) => k + 1)}>
          {route.page === 'home' && <HomePage />}
          {route.page === 'workspaces' && <WorkspacesPage />}
          {route.page === 'open' && <OpenPackagePage />}
          {route.page === 'templates' && <TemplatesPage />}
          {route.page === 'activity' && <ActivityPage />}
          {route.page === 'settings' && <SettingsPage />}
        </ErrorBoundary>
      </main>
    </div>
  );
}
