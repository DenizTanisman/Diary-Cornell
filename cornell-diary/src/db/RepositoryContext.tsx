import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import type { IDiaryRepository } from './IDiaryRepository';
import { SQLiteRepository } from './SQLiteRepository';
import { logger } from '../utils/logger';

const RepositoryCtx = createContext<IDiaryRepository | null>(null);

interface ProviderProps {
  children: ReactNode;
  repository?: IDiaryRepository;
}

export function RepositoryProvider({ children, repository }: ProviderProps) {
  const [repo, setRepo] = useState<IDiaryRepository | null>(repository ?? null);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    if (repository) {
      setRepo(repository);
      return;
    }
    const instance = new SQLiteRepository();
    instance
      .init()
      .then(() => setRepo(instance))
      .catch((err: unknown) => {
        logger.error('repository_init_failed', { error: String(err) });
        setError(err instanceof Error ? err : new Error(String(err)));
      });
  }, [repository]);

  if (error) {
    return (
      <div className="app-error">
        <h1>Veritabanı Hatası</h1>
        <p>{error.message}</p>
      </div>
    );
  }

  if (!repo) {
    return <div className="app-loading">Yükleniyor…</div>;
  }

  return <RepositoryCtx.Provider value={repo}>{children}</RepositoryCtx.Provider>;
}

export function useRepository(): IDiaryRepository {
  const repo = useContext(RepositoryCtx);
  if (!repo) throw new Error('useRepository must be used within RepositoryProvider');
  return repo;
}
