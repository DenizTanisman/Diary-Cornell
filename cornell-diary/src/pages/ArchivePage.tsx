import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { useRepository } from '../db/RepositoryContext';
import { formatTurkishShort, formatDayName } from '../utils/date';
import { useT } from '../locales';
import { logger } from '../utils/logger';

export function ArchivePage() {
  const t = useT();
  const repo = useRepository();
  const [dates, setDates] = useState<string[] | null>(null);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const d = await repo.getAllDates();
        setDates(d);
      } catch (err) {
        logger.error('archive_load_failed', { error: String(err) });
        setError(err instanceof Error ? err : new Error(String(err)));
      }
    })();
  }, [repo]);

  if (error) return <div className="page-container app-error">{error.message}</div>;
  if (dates === null) return <div className="page-container">{t('app.loading')}</div>;

  return (
    <div className="page-container">
      <h1>{t('archive.title')}</h1>
      <p className="cornell-header__counter">{t('archive.count', { count: dates.length })}</p>
      {dates.length === 0 ? (
        <p className="empty-state">{t('archive.empty')}</p>
      ) : (
        <div className="archive-list">
          {dates.map((d) => (
            <Link key={d} to={`/diary/${d}`} className="archive-list__item">
              <span className="archive-list__date">{d}</span>
              <span>
                {formatTurkishShort(d)} — {formatDayName(d)}
              </span>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
