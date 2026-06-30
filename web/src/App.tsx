import { Link, Outlet } from 'react-router-dom';

export function App() {
  return (
    <div className="app">
      <header className="app-header">
        <div className="brand">
          <Link to="/" style={{ color: 'inherit' }}>
            <h1>FlightTrace</h1>
          </Link>
          <span className="brand-sub">mission debugger</span>
        </div>
        <div className="brand-sub">
          local · v0.1 · <span className="mono">demo only</span>
        </div>
      </header>
      <main className="app-main">
        <Outlet />
      </main>
    </div>
  );
}
