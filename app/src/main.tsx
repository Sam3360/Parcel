import React from 'react';
import ReactDOM from 'react-dom/client';
import './styles.css';
import { App } from './App';
import { AppProvider } from './hooks/app';
import { tauriApi } from './lib/tauriApi';
import { setApi } from './lib/api';

setApi(tauriApi);
ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AppProvider api={tauriApi}>
      <App />
    </AppProvider>
  </React.StrictMode>,
);
