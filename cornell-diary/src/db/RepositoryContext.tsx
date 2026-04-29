import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import type { IDiaryRepository } from './IDiaryRepository';
import { SQLiteRepository } from './SQLiteRepository';
import { TauriRepository } from './TauriRepository';
import { logger } from '../utils/logger';

const RepositoryCtx = createContext<IDiaryRepository | null>(null);

/**
 * Default backend selector.
 *
 * - `tauri` (production default): all reads/writes go through Rust via
 *   `invoke('diary_*')`. Rust holds the SQLite handle today, Postgres
 *   tomorrow (FAZ 1.1). This is the path the rest of the migration plan
 *   assumes.
 * - `sqlite` (rollback escape hatch): the legacy direct-from-frontend
 *   SQLiteRepository. Set `VITE_REPOSITORY_BACKEND=sqlite` if Rust's
 *   side breaks during transition; deleted entirely in FAZ 1.3.
 */
function pickDefaultRepository(): IDiaryRepository {
  const flag = (import.meta.env.VITE_REPOSITORY_BACKEND ?? '').toLowerCase();
  if (flag === 'sqlite') {
    return new SQLiteRepository();
  }
  return new TauriRepository();
}

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
    const instance = pickDefaultRepository();
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
