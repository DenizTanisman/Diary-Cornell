import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import App from './App';
import { RepositoryProvider } from './db/RepositoryContext';
import { ErrorBoundary } from './components/common/ErrorBoundary';
import './styles/globals.css';
import './styles/cornell.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <RepositoryProvider>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </RepositoryProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
