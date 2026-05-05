import { invoke } from '@tauri-apps/api/core';
import { useEffect, useRef, useState } from 'react';

interface CloudServiceStatus {
  state: 'idle' | 'starting' | 'running' | 'error';
  pid: number | null;
  lastError: string | null;
  healthy: boolean;
}

const POLL_INTERVAL_MS = 1500;

export function CloudServicePanel() {
  const [status, setStatus] = useState<CloudServiceStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [autoStart, setAutoStart] = useState<boolean | null>(null);
  const pollRef = useRef<number | null>(null);

  const refresh = async () => {
    try {
      const s = await invoke<CloudServiceStatus>('cloud_service_status');
      setStatus(s);
    } catch (e) {
      setStatus(null);
      setError(String(e));
    }
  };

  useEffect(() => {
    void refresh();
    invoke<boolean>('get_auto_start_cloud').then(setAutoStart).catch(() => setAutoStart(false));
    pollRef.current = window.setInterval(() => {
      void refresh();
    }, POLL_INTERVAL_MS);
    return () => {
      if (pollRef.current !== null) window.clearInterval(pollRef.current);
    };
  }, []);

  const toggleAutoStart = async () => {
    if (autoStart === null) return;
    const next = !autoStart;
    setAutoStart(next);
    try {
      await invoke('set_auto_start_cloud', { enabled: next });
    } catch (e) {
      setAutoStart(!next);
      setError(String(e));
    }
  };

  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const s = await invoke<CloudServiceStatus>('start_cloud_service');
      setStatus(s);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      void refresh();
    }
  };

  const stop = async () => {
    setBusy(true);
    setError(null);
    try {
      const s = await invoke<CloudServiceStatus>('stop_cloud_service');
      setStatus(s);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      void refresh();
    }
  };

  const stateLabel = (() => {
    if (!status) return 'Yükleniyor…';
    if (status.healthy && status.state === 'running') return '✓ Cloud aktif (port 5001)';
    if (status.state === 'starting') return '⏳ Başlatılıyor…';
    if (status.state === 'error') return `✗ Hata: ${status.lastError ?? 'bilinmeyen'}`;
    if (status.healthy) return '✓ Cloud aktif (Diary başlatmadı, dış servis)';
    return '○ Cloud kapalı';
  })();

  const isRunning = status?.healthy === true;
  const showStart = !isRunning && status?.state !== 'starting';

  return (
    <div
      className="cloud-service-panel"
      style={{
        marginTop: '1rem',
        padding: '0.8rem 1rem',
        border: '1px solid rgba(0,0,0,0.12)',
        borderRadius: 8,
      }}
    >
      <h3 style={{ margin: 0, marginBottom: '0.4rem' }}>Cloud Servisi</h3>
      <p style={{ margin: '0 0 0.6rem 0', fontSize: '0.85rem', opacity: 0.7 }}>
        Senkronizasyon için yerel Cloud sunucusu (port 5001). Postgres ve uvicorn'ı tek butonla
        başlatır; Diary kapanınca otomatik durur.
      </p>
      {error && (
        <div role="alert" style={{ color: '#BA2222', marginBottom: '0.4rem', fontSize: '0.85rem' }}>
          {error}
        </div>
      )}
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.8rem' }}>
        <span style={{ flex: 1 }}>{stateLabel}</span>
        {showStart ? (
          <button onClick={start} disabled={busy}>
            {busy ? '…' : "Cloud'u Başlat"}
          </button>
        ) : (
          <button onClick={stop} disabled={busy}>
            {busy ? '…' : "Cloud'u Durdur"}
          </button>
        )}
      </div>
      {status?.pid && (
        <div style={{ fontSize: '0.75rem', opacity: 0.55, marginTop: '0.4rem' }}>
          PID: {status.pid}
        </div>
      )}
      <label
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '0.5rem',
          marginTop: '0.6rem',
          fontSize: '0.85rem',
          cursor: 'pointer',
        }}
      >
        <input
          type="checkbox"
          checked={autoStart === true}
          disabled={autoStart === null}
          onChange={toggleAutoStart}
        />
        <span>Diary açıldığında Cloud'u otomatik başlat</span>
      </label>
    </div>
  );
}
