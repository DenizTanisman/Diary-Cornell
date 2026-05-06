import { invoke } from '@tauri-apps/api/core';
import { useEffect, useRef, useState } from 'react';

interface CloudServiceStatus {
  state: 'idle' | 'starting' | 'running' | 'error';
  pid: number | null;
  lastError: string | null;
  healthy: boolean;
}

const POLL_INTERVAL_MS = 1500;
const CLOUD_PORT = 5001;

export function CloudServicePanel() {
  const [status, setStatus] = useState<CloudServiceStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [autoStart, setAutoStart] = useState<boolean | null>(null);
  const [lanAddresses, setLanAddresses] = useState<string[]>([]);
  const [copied, setCopied] = useState<string | null>(null);
  const pollRef = useRef<number | null>(null);
  const copyTimer = useRef<number | null>(null);

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
    invoke<string[]>('get_lan_addresses')
      .then(setLanAddresses)
      .catch(() => setLanAddresses([]));
    pollRef.current = window.setInterval(() => {
      void refresh();
    }, POLL_INTERVAL_MS);
    return () => {
      if (pollRef.current !== null) window.clearInterval(pollRef.current);
      if (copyTimer.current !== null) window.clearTimeout(copyTimer.current);
    };
  }, []);

  const copyAddress = async (full: string) => {
    try {
      await navigator.clipboard.writeText(full);
      setCopied(full);
      if (copyTimer.current !== null) window.clearTimeout(copyTimer.current);
      copyTimer.current = window.setTimeout(() => setCopied(null), 1500);
    } catch {
      // Clipboard rejected — fall back silently; the URL is visible
      // in the row, the user can still long-press.
    }
  };

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
    <div className="cloud-service-panel">
      <h3 className="cloud-service-panel__title">Cloud Servisi</h3>
      <p className="cloud-service-panel__description">
        Senkronizasyon için yerel Cloud sunucusu (port 5001). Postgres ve uvicorn'ı tek butonla
        başlatır; Diary kapanınca otomatik durur.
      </p>
      {error && (
        <div role="alert" className="cloud-service-panel__error">
          {error}
        </div>
      )}
      <div className="cloud-service-panel__row">
        <span className="cloud-service-panel__state">{stateLabel}</span>
        {showStart ? (
          <button className="cloud-service-panel__button" onClick={start} disabled={busy}>
            {busy ? '…' : "Cloud'u Başlat"}
          </button>
        ) : (
          <button className="cloud-service-panel__button" onClick={stop} disabled={busy}>
            {busy ? '…' : "Cloud'u Durdur"}
          </button>
        )}
      </div>
      {status?.pid && <div className="cloud-service-panel__pid">PID: {status.pid}</div>}
      {isRunning && lanAddresses.length > 0 && (
        <div className="cloud-service-panel__lan">
          <div className="cloud-service-panel__lan-label">
            📱 Telefondan / diğer cihazlardan erişim için:
          </div>
          <ul className="cloud-service-panel__lan-list">
            {lanAddresses.map((ip) => {
              const url = `http://${ip}:${CLOUD_PORT}`;
              return (
                <li key={ip} className="cloud-service-panel__lan-item">
                  <code>{url}</code>
                  <button
                    type="button"
                    className="cloud-service-panel__lan-copy"
                    onClick={() => void copyAddress(url)}
                    aria-label={`${url} kopyala`}
                  >
                    {copied === url ? '✓ Kopyalandı' : 'Kopyala'}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      <label className="cloud-service-panel__autostart">
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
