import { useEffect, useState } from 'react';
import { Link, Outlet } from 'react-router-dom';

import { ThemeToggle } from './components/ThemeToggle';
import { getHealth } from './api/runs';

const BASE_TITLE = 'FlightTrace — mission debugger';

export function App() {
  const [version, setVersion] = useState<string | null>(null);

  // Read the CLI/desktop version straight off /health. The same endpoint
  // is exposed in every run mode (dev, packaged CLI, Tauri desktop), so
  // this covers all of them with one code path.
  useEffect(() => {
    const controller = new AbortController();
    getHealth(controller.signal)
      .then((data) => setVersion(data.version))
      .catch(() => {
        /* leave version null; header shows the fallback label. */
      });
    return () => controller.abort();
  }, []);

  useEffect(() => {
    document.title = version ? `FlightTrace v${version}` : BASE_TITLE;
  }, [version]);

  return (
    <div className="app">
      <header className="app-header">
        <div className="brand">
          <img
            className="logo"
            src="/icon.png"
            alt=""
            aria-hidden="true"
            draggable={false}
          />
          <Link to="/">
            <h1>FlightTrace</h1>
          </Link>
          <span className="brand-sub">mission debugger</span>
        </div>
        <div className="app-header-right">
          <div className="brand-sub">
            local · <span className="app-version">{version ? `v${version}` : 'v0.1'}</span> ·{' '}
            <span className="mono">demo only</span>
          </div>
          <ThemeToggle />
        </div>
      </header>
      <main className="app-main">
        <Outlet />
      </main>
    </div>
  );
}
