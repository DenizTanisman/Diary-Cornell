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
    <div className="auto-sync-toggle">
      <h3 className="auto-sync-toggle__title">Otomatik Senkronizasyon</h3>
      <p className="auto-sync-toggle__description">
        Açıkken Diary, Cloud'a 2 dakikada bir kendiliğinden bağlanır ve değişiklikleri
        senkronize eder. Yeniden başlatmadan kapanır/açılır.
      </p>
      {error && (
        <div role="alert" className="auto-sync-toggle__error">
          {error}
        </div>
      )}
      <label className="auto-sync-toggle__label">
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
