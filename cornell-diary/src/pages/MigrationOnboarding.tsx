/**
 * One-shot migration page (FAZ 1.2 of diary_prompt_v2.md).
 *
 * Lifetime: this page exists only between FAZ 1.2 and FAZ 1.3. Once the
 * user has run the migration once, FAZ 1.3 deletes both this file and the
 * underlying `migrate_sqlite_to_postgres` Tauri command.
 *
 * Two buttons, no surprises:
 *  - **Dry Run** reads the SQLite snapshot and prints what *would* be
 *    inserted. Nothing in Postgres changes.
 *  - **Migrate** actually runs the transactional copy. Re-runs are safe
 *    (rows already in Postgres are skipped, sync_log is append-only).
 */

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface TableCounts {
  diaryEntries: number;
  syncLogs: number;
  appSettings: number;
}

interface MigrationReport {
  dryRun: boolean;
  sourceCounts: TableCounts;
  targetCounts: TableCounts;
  inserted: TableCounts;
  skipped: TableCounts;
  durationMs: number;
  success: boolean;
  error: string;
}

const DEFAULT_SQLITE_PATH =
  '/Users/ismaildeniz/Library/Application Support/com.deniz.cornelldiary/cornell_diary.db';

export function MigrationOnboarding() {
  const [sqlitePath, setSqlitePath] = useState<string>(DEFAULT_SQLITE_PATH);
  const [busy, setBusy] = useState(false);
  const [phase, setPhase] = useState<'idle' | 'dry' | 'real'>('idle');
  const [report, setReport] = useState<MigrationReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Pre-fill from env if the user passed SQLITE_LEGACY_PATH at build time;
    // VITE_SQLITE_LEGACY_PATH is the runtime-readable alias.
    const fromEnv = import.meta.env.VITE_SQLITE_LEGACY_PATH;
    if (typeof fromEnv === 'string' && fromEnv.trim()) {
      setSqlitePath(fromEnv);
    }
  }, []);

  async function run(dryRun: boolean) {
    setBusy(true);
    setError(null);
    setPhase(dryRun ? 'dry' : 'real');
    try {
      const r = await invoke<MigrationReport>('migrate_sqlite_to_postgres', {
        sqlitePath,
        dryRun,
      });
      setReport(r);
      if (!r.success) {
        setError(r.error || 'Migration reported success=false');
      }
    } catch (e) {
      const env = e as { code?: string; message?: string } | string;
      const msg =
        typeof env === 'string'
          ? env
          : env.message
            ? `[${env.code ?? 'error'}] ${env.message}`
            : JSON.stringify(env);
      setError(msg);
      setReport(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <main style={containerStyle}>
      <h1 style={{ fontSize: '1.4rem', marginBottom: '0.5rem' }}>SQLite → Postgres Geçişi</h1>
      <p style={{ color: '#5A5A5A', marginBottom: '1rem', maxWidth: '52ch' }}>
        Eski SQLite günlüğünüzü Postgres'e taşıyın. Önce <strong>Kuru Çalıştırma</strong>
        ile kaç satır taşınacağını görün; ardından gerçek geçişi başlatın. Yanlışlıkla
        tekrar tıklarsanız, mevcut günler atlanır — veriniz asla üzerine yazılmaz.
      </p>

      <label style={labelStyle}>
        SQLite dosya yolu
        <input
          type="text"
          value={sqlitePath}
          onChange={(e) => setSqlitePath(e.target.value)}
          disabled={busy}
          style={inputStyle}
          data-testid="migration-sqlite-path"
        />
      </label>

      <div style={{ display: 'flex', gap: '0.75rem', marginTop: '1rem' }}>
        <button
          type="button"
          onClick={() => run(true)}
          disabled={busy}
          style={{ ...buttonStyle, background: '#E8E4DC' }}
          data-testid="migration-dry-run"
        >
          {busy && phase === 'dry' ? 'Sayılıyor…' : 'Kuru Çalıştır (Dry Run)'}
        </button>
        <button
          type="button"
          onClick={() => run(false)}
          disabled={busy}
          style={{ ...buttonStyle, background: '#0A1628', color: '#FAF7F2' }}
          data-testid="migration-real-run"
        >
          {busy && phase === 'real' ? 'Aktarılıyor…' : 'Gerçek Geçişi Başlat'}
        </button>
      </div>

      {error && (
        <div style={errorStyle} data-testid="migration-error">
          <strong>Hata:</strong> {error}
        </div>
      )}

      {report && (
        <section style={reportStyle} data-testid="migration-report">
          <h2 style={{ fontSize: '1.1rem', marginBottom: '0.5rem' }}>
            {report.dryRun ? 'Kuru Çalıştırma Raporu' : 'Geçiş Raporu'}
          </h2>
          <p style={{ color: '#5A5A5A', marginBottom: '0.5rem' }}>
            Süre: {report.durationMs} ms ·{' '}
            {report.success ? <span style={{ color: '#3B6D11' }}>başarılı</span> : <span style={{ color: '#BA2222' }}>başarısız</span>}
          </p>
          <table style={tableStyle}>
            <thead>
              <tr>
                <th>Tablo</th>
                <th>SQLite</th>
                <th>Postgres (öncesi)</th>
                <th>Eklendi</th>
                <th>Atlandı</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>diary_entries</td>
                <td>{report.sourceCounts.diaryEntries}</td>
                <td>{report.targetCounts.diaryEntries}</td>
                <td>{report.inserted.diaryEntries}</td>
                <td>{report.skipped.diaryEntries}</td>
              </tr>
              <tr>
                <td>sync_log</td>
                <td>{report.sourceCounts.syncLogs}</td>
                <td>{report.targetCounts.syncLogs}</td>
                <td>{report.inserted.syncLogs}</td>
                <td>{report.skipped.syncLogs}</td>
              </tr>
              <tr>
                <td>app_settings</td>
                <td>{report.sourceCounts.appSettings}</td>
                <td>{report.targetCounts.appSettings}</td>
                <td>{report.inserted.appSettings}</td>
                <td>{report.skipped.appSettings}</td>
              </tr>
            </tbody>
          </table>
        </section>
      )}
    </main>
  );
}

const containerStyle: React.CSSProperties = {
  maxWidth: '720px',
  margin: '2rem auto',
  padding: '1.5rem 1rem',
  fontFamily: 'system-ui, -apple-system, sans-serif',
};

const labelStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '0.25rem',
  fontSize: '0.85rem',
  color: '#1A1A1A',
};

const inputStyle: React.CSSProperties = {
  padding: '0.5rem 0.75rem',
  border: '1px solid #D0CCC5',
  borderRadius: '4px',
  fontSize: '0.9rem',
  fontFamily: 'monospace',
  background: '#FAF7F2',
};

const buttonStyle: React.CSSProperties = {
  padding: '0.6rem 1rem',
  border: '1px solid #2C3E50',
  borderRadius: '4px',
  fontSize: '0.95rem',
  cursor: 'pointer',
};

const errorStyle: React.CSSProperties = {
  marginTop: '1rem',
  padding: '0.75rem 1rem',
  background: '#F4D6CB',
  border: '1px solid #BA2222',
  borderRadius: '4px',
  color: '#561010',
};

const reportStyle: React.CSSProperties = {
  marginTop: '1.5rem',
  padding: '1rem',
  border: '1px solid #D0CCC5',
  borderRadius: '6px',
  background: '#FAF7F2',
};

const tableStyle: React.CSSProperties = {
  width: '100%',
  borderCollapse: 'collapse',
  fontSize: '0.9rem',
};
