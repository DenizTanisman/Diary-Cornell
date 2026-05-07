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
import { usePlatform } from '../../hooks/usePlatform';
import type { Platform } from '../../hooks/usePlatform';
import type { ConnectReport, SyncReport } from '../../types/cloudSync';

export function CloudSyncPanel() {
  const { status, error: statusError, refresh } = useSyncStatus();
  const { platform } = usePlatform();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [deviceLabel, setDeviceLabel] = useState(() => detectDeviceLabel(platform));
  const [busy, setBusy] = useState<'idle' | 'connect' | 'trigger' | 'disconnect' | 'forgot' | 'reset'>('idle');
  const [actionError, setActionError] = useState<string | null>(null);
  const [lastReport, setLastReport] = useState<SyncReport | null>(null);
  const [connectReport, setConnectReport] = useState<ConnectReport | null>(null);
  const [authMode, setAuthMode] = useState<'login' | 'forgot' | 'reset'>('login');
  const [forgotEmail, setForgotEmail] = useState('');
  const [resetToken, setResetToken] = useState('');
  const [resetNewPassword, setResetNewPassword] = useState('');
  const [authNotice, setAuthNotice] = useState<string | null>(null);

  const isConnected = status?.enabled ?? false;

  async function onConnect() {
    setBusy('connect');
    setActionError(null);
    try {
      const r = await invoke<ConnectReport>('connect_cloud', {
        username,
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

  async function onForgot() {
    setBusy('forgot');
    setActionError(null);
    setAuthNotice(null);
    try {
      await invoke<void>('forgot_password_cloud', { email: forgotEmail });
      // Cloud always returns 200 — present the same wording either way so we
      // don't leak which addresses are registered.
      setAuthNotice(
        'E-posta gönderildi. Gelen kutunu kontrol et; bağlantı 60 dakika geçerli.',
      );
      setAuthMode('reset');
    } catch (e) {
      setActionError(extractMessage(e));
    } finally {
      setBusy('idle');
    }
  }

  async function onReset() {
    setBusy('reset');
    setActionError(null);
    setAuthNotice(null);
    try {
      await invoke<void>('reset_password_cloud', {
        token: resetToken,
        newPassword: resetNewPassword,
      });
      setAuthNotice('Şifre güncellendi. Yeni şifrenle giriş yapabilirsin.');
      setResetToken('');
      setResetNewPassword('');
      setAuthMode('login');
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
    <section className="cloud-sync-panel" data-testid="cloud-sync-panel">
      <h2 className="sync-card__title">Cloud Senkronizasyonu</h2>

      {!isConnected && authMode === 'login' && (
        <form
          className="cloud-sync-panel__form"
          onSubmit={(e) => {
            e.preventDefault();
            void onConnect();
          }}
        >
          <p className="sync-card__description">
            Giriş bilgilerinizle Cloud'a bağlanın. Şifre yalnızca giriş çağrısında
            kullanılır, hiçbir yere yazılmaz.
          </p>
          <label className="cloud-sync-panel__field">
            Kullanıcı adı
            <input
              type="text"
              required
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              disabled={busy !== 'idle'}
              data-testid="cloud-username"
              className="cloud-sync-panel__input"
            />
          </label>
          <label className="cloud-sync-panel__field">
            Şifre
            <input
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={busy !== 'idle'}
              data-testid="cloud-password"
              className="cloud-sync-panel__input"
            />
          </label>
          <label className="cloud-sync-panel__field">
            Cihaz etiketi
            <input
              type="text"
              value={deviceLabel}
              onChange={(e) => setDeviceLabel(e.target.value)}
              disabled={busy !== 'idle'}
              className="cloud-sync-panel__input"
            />
          </label>
          <button
            type="submit"
            className="sync-card__button"
            disabled={busy !== 'idle' || !username || !password}
            data-testid="cloud-connect"
          >
            {busy === 'connect' ? 'Bağlanılıyor…' : 'Cloud\'a Bağlan'}
          </button>
          <div className="cloud-sync-panel__link-row">
            <button
              type="button"
              className="cloud-sync-panel__link"
              onClick={() => {
                setAuthMode('forgot');
                setActionError(null);
                setAuthNotice(null);
              }}
              data-testid="cloud-forgot-link"
            >
              Şifremi unuttum
            </button>
            <button
              type="button"
              className="cloud-sync-panel__link"
              onClick={() => {
                setAuthMode('reset');
                setActionError(null);
                setAuthNotice(null);
              }}
              data-testid="cloud-have-token-link"
            >
              Sıfırlama kodum var
            </button>
          </div>
        </form>
      )}

      {!isConnected && authMode === 'forgot' && (
        <form
          className="cloud-sync-panel__form"
          onSubmit={(e) => {
            e.preventDefault();
            void onForgot();
          }}
        >
          <p className="sync-card__description">
            Hesabınızdaki e-postayı girin. Doğrulanmış adreslere sıfırlama bağlantısı
            gönderilir; başkasının adresine spam yapmamak için diğer durumlarda da aynı
            mesajı görürsünüz.
          </p>
          <label className="cloud-sync-panel__field">
            E-posta
            <input
              type="email"
              required
              autoComplete="email"
              value={forgotEmail}
              onChange={(e) => setForgotEmail(e.target.value)}
              disabled={busy !== 'idle'}
              data-testid="cloud-forgot-email"
              className="cloud-sync-panel__input"
            />
          </label>
          <div className="cloud-sync-panel__actions">
            <button
              type="submit"
              className="sync-card__button"
              disabled={busy !== 'idle' || !forgotEmail}
              data-testid="cloud-forgot-submit"
            >
              {busy === 'forgot' ? 'Gönderiliyor…' : 'Sıfırlama bağlantısı yolla'}
            </button>
            <button
              type="button"
              className="sync-card__button cloud-sync-panel__button--secondary"
              onClick={() => {
                setAuthMode('login');
                setActionError(null);
                setAuthNotice(null);
              }}
              disabled={busy !== 'idle'}
            >
              Vazgeç
            </button>
          </div>
        </form>
      )}

      {!isConnected && authMode === 'reset' && (
        <form
          className="cloud-sync-panel__form"
          onSubmit={(e) => {
            e.preventDefault();
            void onReset();
          }}
        >
          <p className="sync-card__description">
            E-postadaki bağlantıdaki <code>token=</code> sonrasındaki kodu yapıştır ve
            yeni şifreni belirle. Token tek kullanımlık ve 60 dakika geçerli.
          </p>
          <label className="cloud-sync-panel__field">
            Sıfırlama tokenı
            <input
              type="text"
              required
              value={resetToken}
              onChange={(e) => setResetToken(e.target.value)}
              disabled={busy !== 'idle'}
              data-testid="cloud-reset-token"
              className="cloud-sync-panel__input"
            />
          </label>
          <label className="cloud-sync-panel__field">
            Yeni şifre
            <input
              type="password"
              required
              autoComplete="new-password"
              value={resetNewPassword}
              onChange={(e) => setResetNewPassword(e.target.value)}
              disabled={busy !== 'idle'}
              data-testid="cloud-reset-password"
              className="cloud-sync-panel__input"
            />
          </label>
          <div className="cloud-sync-panel__actions">
            <button
              type="submit"
              className="sync-card__button"
              disabled={busy !== 'idle' || !resetToken || !resetNewPassword}
              data-testid="cloud-reset-submit"
            >
              {busy === 'reset' ? 'Sıfırlanıyor…' : 'Şifreyi sıfırla'}
            </button>
            <button
              type="button"
              className="sync-card__button cloud-sync-panel__button--secondary"
              onClick={() => {
                setAuthMode('login');
                setActionError(null);
                setAuthNotice(null);
              }}
              disabled={busy !== 'idle'}
            >
              Vazgeç
            </button>
          </div>
        </form>
      )}

      {isConnected && (
        <div className="cloud-sync-panel__form">
          {connectReport && (
            <p className="sync-card__description" data-testid="cloud-connected">
              {connectReport.journalName} jurnaline bağlandın · peer{' '}
              <code>{connectReport.peerId}</code>
            </p>
          )}
          <dl className="cloud-sync-panel__status-grid">
            <dt>Çevrimiçi</dt>
            <dd>{status?.online ? 'Evet' : 'Hayır'}</dd>
            <dt>Son pull</dt>
            <dd>{formatTime(status?.lastPullAt)}</dd>
            <dt>Son push</dt>
            <dd>{formatTime(status?.lastPushAt)}</dd>
            <dt>Bekleyen değişiklik</dt>
            <dd>{status?.dirtyCount ?? 0}</dd>
          </dl>
          <div className="cloud-sync-panel__actions">
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
              className="sync-card__button cloud-sync-panel__button--danger"
              onClick={() => void onDisconnect()}
              disabled={busy !== 'idle'}
              data-testid="cloud-disconnect"
            >
              {busy === 'disconnect' ? 'Bağlantı kesiliyor…' : 'Bağlantıyı Kes'}
            </button>
          </div>
        </div>
      )}

      {authNotice && (
        <p
          role="status"
          className="cloud-sync-panel__notice"
          data-testid="cloud-auth-notice"
        >
          {authNotice}
        </p>
      )}

      {(actionError ?? statusError) && (
        <p
          role="alert"
          className="cloud-sync-panel__error"
          data-testid="cloud-error"
        >
          {actionError ?? statusError}
        </p>
      )}

      {lastReport && (
        <p
          className="empty-state"
          role="status"
          data-testid="cloud-last-report"
        >
          Son senkronizasyon: {lastReport.pulled} çekildi · {lastReport.pushed} gönderildi ·{' '}
          {lastReport.conflictsCloudWon + lastReport.conflictsLocalWon} çakışma · {lastReport.durationMs} ms
        </p>
      )}
    </section>
  );
}

function detectDeviceLabel(platform: Platform): string {
  // Tauri's plugin-os reports the host OS as a friendly token; on Android
  // it returns 'android' even though the WebView's navigator.platform
  // string is 'Linux aarch64' (kernel-level). We branch on the Tauri
  // value first so mobile builds don't ship a "Diary on Linux aarch64"
  // label to Cloud, then fall back to navigator.platform for the
  // browser preview / vitest path.
  switch (platform) {
    case 'android':
      return 'Diary on Android';
    case 'ios':
      return 'Diary on iOS';
    case 'macos':
      return 'Diary on Mac';
    case 'windows':
      return 'Diary on Windows';
    case 'linux':
      return 'Diary on Linux';
    default:
      if (typeof navigator !== 'undefined' && navigator.platform) {
        return `Diary on ${navigator.platform}`;
      }
      return 'Diary';
  }
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
