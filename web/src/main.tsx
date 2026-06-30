import React from 'react';
import ReactDOM from 'react-dom/client';

import { AppRouter } from './router';
import './styles/globals.css';

const root = document.getElementById('root');
if (!root) {
  throw new Error('missing #root element in index.html');
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <AppRouter />
  </React.StrictMode>,
);
