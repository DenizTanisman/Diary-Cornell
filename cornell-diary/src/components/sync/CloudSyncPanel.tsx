/**
 * Cloud sync section embedded inside the existing SyncPage. Lets the user:
 *  - connect (email + password + device label) when no token is held,
 *  - see status (online, last pull/push, dirty count) when a token is held,
 *  - trigger a manual sync,
 *  - disconnect (clears tokens; engine stops trying).
 *
 * The QR / JSON manual sync that already lives on SyncPage is untouched —
 * Cloud sync is additive.
 */
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import { useSyncStatus } from '../../hooks/useSyncStatus';
import type { ConnectReport, SyncReport } from '../../types/cloudSync';

export function CloudSyncPanel() {
  const { status, error: statusError, refresh } = useSyncStatus();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [deviceLabel, setDeviceLabel] = useState(detectDeviceLabel());
  const [busy, setBusy] = useState<'idle' | 'connect' | 'trigger' | 'disconnect'>('idle');
  const [actionError, setActionError] = useState<string | null>(null);
  const [lastReport, setLastReport] = useState<SyncReport | null>(null);
  const [connectReport, setConnectReport] = useState<ConnectReport | null>(null);

  const isConnected = status?.enabled ?? false;

  async function onConnect() {
    setBusy('connect');
    setActionError(null);
    try {
      const r = await invoke<ConnectReport>('connect_cloud', {
        email,
        password,
        deviceLabel,
      });
      setConnectReport(r);
      // Don't keep the password in state any longer than the call.
      setPassword('');
      refresh();
    } catch (e) {
      setActionError(extractMessage(e));
    } finally {
      setBusy('idle');
    }
  }

  async function onTrigger() {
    setBusy('trigger');
    setActionError(null);
    try {
      const r = await invoke<SyncReport>('trigger_sync');
      setLastReport(r);
      refresh();
    } catch (e) {
      setActionError(extractMessage(e));
    } finally {
      setBusy('idle');
    }
  }

  async function onDisconnect() {
    setBusy('disconnect');
    setActionError(null);
    try {
      await invoke<void>('disconnect_cloud');
      setConnectReport(null);
      setLastReport(null);
      refresh();
    } catch (e) {
      setActionError(extractMessage(e));
    } finally {
      setBusy('idle');
    }
  }

  return (
    <section className="sync-card" data-testid="cloud-sync-panel">
      <h2 className="sync-card__title">Cloud Senkronizasyonu</h2>

      {!isConnected && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void onConnect();
          }}
          style={formStyle}
        >
          <p className="sync-card__description">
            Giriş bilgilerinizle Cloud'a bağlanın. Şifre yalnızca giriş çağrısında
            kullanılır, hiçbir yere yazılmaz.
          </p>
          <label style={labelStyle}>
            E-posta
            <input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              disabled={busy !== 'idle'}
              data-testid="cloud-email"
              style={inputStyle}
            />
          </label>
          <label style={labelStyle}>
            Şifre
            <input
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={busy !== 'idle'}
              data-testid="cloud-password"
              style={inputStyle}
            />
          </label>
          <label style={labelStyle}>
            Cihaz etiketi
            <input
              type="text"
              value={deviceLabel}
              onChange={(e) => setDeviceLabel(e.target.value)}
              disabled={busy !== 'idle'}
              style={inputStyle}
            />
          </label>
          <button
            type="submit"
            className="sync-card__button"
            disabled={busy !== 'idle' || !email || !password}
            data-testid="cloud-connect"
          >
            {busy === 'connect' ? 'Bağlanılıyor…' : 'Cloud\'a Bağlan'}
          </button>
        </form>
      )}

      {isConnected && (
        <div style={formStyle}>
          {connectReport && (
            <p className="sync-card__description" data-testid="cloud-connected">
              {connectReport.journalName} jurnaline bağlandın · peer{' '}
              <code>{connectReport.peerId}</code>
            </p>
          )}
          <dl style={dlStyle}>
            <dt>Çevrimiçi</dt>
            <dd>{status?.online ? 'Evet' : 'Hayır'}</dd>
            <dt>Son pull</dt>
            <dd>{formatTime(status?.lastPullAt)}</dd>
            <dt>Son push</dt>
            <dd>{formatTime(status?.lastPushAt)}</dd>
            <dt>Bekleyen değişiklik</dt>
            <dd>{status?.dirtyCount ?? 0}</dd>
          </dl>
          <div style={{ display: 'flex', gap: '0.6rem' }}>
            <button
              type="button"
              className="sync-card__button"
              onClick={() => void onTrigger()}
              disabled={busy !== 'idle'}
              data-testid="cloud-trigger"
            >
              {busy === 'trigger' ? 'Senkronize ediliyor…' : 'Şimdi Senkronize Et'}
            </button>
            <button
              type="button"
              className="sync-card__button"
              onClick={() => void onDisconnect()}
              disabled={busy !== 'idle'}
              data-testid="cloud-disconnect"
              style={{ borderColor: '#BA2222', color: '#BA2222' }}
            >
              {busy === 'disconnect' ? 'Bağlantı kesiliyor…' : 'Bağlantıyı Kes'}
            </button>
          </div>
        </div>
      )}

      {(actionError ?? statusError) && (
        <p
          role="alert"
          data-testid="cloud-error"
          style={{
            marginTop: '0.75rem',
            padding: '0.5rem 0.75rem',
            background: '#F4D6CB',
            borderRadius: '4px',
            color: '#561010',
            fontSize: '0.85rem',
          }}
        >
          {actionError ?? statusError}
        </p>
      )}

      {lastReport && (
        <p
          className="empty-state"
          role="status"
          data-testid="cloud-last-report"
          style={{ marginTop: '0.75rem' }}
        >
          Son senkronizasyon: {lastReport.pulled} çekildi · {lastReport.pushed} gönderildi ·{' '}
          {lastReport.conflictsCloudWon + lastReport.conflictsLocalWon} çakışma · {lastReport.durationMs} ms
        </p>
      )}
    </section>
  );
}

function detectDeviceLabel(): string {
  // Fallback only — Tauri provides a real hostname via tauri-plugin-os if
  // we want to invoke('hostname'); the user can edit before submitting.
  if (typeof navigator !== 'undefined' && navigator.platform) {
    return `Diary on ${navigator.platform}`;
  }
  return 'Diary';
}

function formatTime(iso: string | null | undefined): string {
  if (!iso) return '—';
  try {
    return new Date(iso).toLocaleString('tr-TR');
  } catch {
    return iso;
  }
}

function extractMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object') {
    const env = e as { code?: string; message?: string };
    if (env.message) return env.code ? `[${env.code}] ${env.message}` : env.message;
  }
  return 'unknown error';
}

const formStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '0.6rem',
  padding: '0.5rem 0',
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
  background: '#FAF7F2',
};

const dlStyle: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'auto 1fr',
  columnGap: '0.75rem',
  rowGap: '0.25rem',
  fontSize: '0.85rem',
  margin: '0.5rem 0',
};
