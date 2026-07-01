import { Link, Outlet } from 'react-router-dom';

import { ThemeToggle } from './components/ThemeToggle';

export function App() {
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
            local · v0.1 · <span className="mono">demo only</span>
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
