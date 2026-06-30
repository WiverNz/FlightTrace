import { createBrowserRouter, Link, RouterProvider } from 'react-router-dom';

import { App } from './App';
import { RunsListPage } from './pages/RunsListPage';
import { RunDetailPage } from './pages/RunDetailPage';

const router = createBrowserRouter([
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <RunsListPage /> },
      { path: 'runs/:id', element: <RunDetailPage /> },
      {
        path: '*',
        element: (
          <div className="section">
            <div className="section-body">
              <h2 style={{ marginTop: 0 }}>Page not found</h2>
              <p className="muted">
                That route does not exist in FlightTrace.
              </p>
              <Link to="/" className="button">
                ← back to runs
              </Link>
            </div>
          </div>
        ),
      },
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
