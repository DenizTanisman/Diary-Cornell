import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

export function AutoSyncToggle() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>('get_auto_sync_enabled')
      .then((v) => {
        if (!cancelled) setEnabled(v);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const toggle = async () => {
    if (enabled === null) return;
    const next = !enabled;
    setBusy(true);
    setError(null);
    // Optimistic flip — the IPC will roll us back on error.
    setEnabled(next);
    try {
      await invoke('set_auto_sync_enabled', { enabled: next });
    } catch (e) {
      setEnabled(!next);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="auto-sync-toggle" style={{ marginTop: '1rem' }}>
      <h3 style={{ marginTop: 0, marginBottom: '0.4rem' }}>Otomatik Senkronizasyon</h3>
      <p style={{ margin: '0 0 0.6rem 0', fontSize: '0.85rem', opacity: 0.7 }}>
        Açıkken Diary, Cloud'a 2 dakikada bir kendiliğinden bağlanır ve değişiklikleri
        senkronize eder. Yeniden başlatmadan kapanır/açılır.
      </p>
      {error && (
        <div role="alert" style={{ color: '#BA2222', marginBottom: '0.4rem', fontSize: '0.85rem' }}>
          {error}
        </div>
      )}
      <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', cursor: 'pointer' }}>
        <input
          type="checkbox"
          checked={enabled === true}
          disabled={enabled === null || busy}
          onChange={toggle}
        />
        <span>Otomatik senkronizasyon (2 dakikada bir)</span>
      </label>
    </div>
  );
}
