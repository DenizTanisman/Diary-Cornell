import { useEffect, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { platform } from '@tauri-apps/plugin-os';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useRepository } from '../../db/RepositoryContext';
import { exportToJSON, serializeExport } from '../../sync/exporter';
import { getDeviceId } from '../../utils/deviceId';
import { Modal } from '../ui/Modal';
import { useT } from '../../locales';
import { logger } from '../../utils/logger';

interface Props {
  onClose: () => void;
}

type Status = 'idle' | 'working' | 'done' | 'error';

export function ExportDialog({ onClose }: Props) {
  const t = useT();
  const repo = useRepository();
  const [status, setStatus] = useState<Status>('idle');
  const [message, setMessage] = useState<string | null>(null);
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [isMobile, setIsMobile] = useState(false);

  useEffect(() => {
    try {
      const p = platform();
      setIsMobile(p === 'android' || p === 'ios');
    } catch {
      setIsMobile(false);
    }
  }, []);

  const handleExport = async () => {
    setStatus('working');
    setMessage(null);
    setSavedPath(null);
    try {
      const entries = await repo.getAll();
      const deviceId = await getDeviceId();
      const file = await exportToJSON(entries, deviceId);
      const content = await serializeExport(file);
      const fileName = `cornell-diary-${new Date().toISOString().slice(0, 10)}.json`;

      // Unified path: plugin-dialog's save() uses the native picker on every
      // platform. On Android it opens the Storage Access Framework, so the
      // user picks Downloads / Drive / etc. themselves — no scoped-storage
      // permission dance, no plugin-opener Android deserialization bug.
      const path = await save({
        defaultPath: fileName,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (!path) {
        setStatus('idle');
        return;
      }

      await writeTextFile(path, content);

      setSavedPath(path);
      setStatus('done');
      setMessage(
        isMobile
          ? `✔ ${entries.length} kayıt seçtiğin konuma yazıldı`
          : `✔ ${entries.length} kayıt dışa aktarıldı`,
      );
    } catch (err) {
      logger.error('export_failed', { error: String(err) });
      setStatus('error');
      setMessage(String(err));
    }
  };

  const revealInFinder = async () => {
    if (!savedPath) return;
    try {
      await revealItemInDir(savedPath);
    } catch (err) {
      logger.warn('reveal_failed', { error: String(err) });
    }
  };

  const copyPath = async () => {
    if (!savedPath) return;
    try {
      await writeText(savedPath);
      setMessage(`${message ?? ''}\n📋 Yol panoya kopyalandı`);
    } catch (err) {
      logger.warn('clipboard_copy_failed', { error: String(err) });
    }
  };

  return (
    <Modal
      title={t('sync.exportTitle')}
      onClose={onClose}
      actions={
        <>
          <button className="modal__close" onClick={onClose}>
            Kapat
          </button>
          {status !== 'done' ? (
            <button
              className="modal__primary"
              onClick={handleExport}
              disabled={status === 'working'}
            >
              {status === 'working' ? '…' : t('sync.exportAction')}
            </button>
          ) : null}
        </>
      }
    >
      <p>{t('sync.exportDescription')}</p>
      {message ? <p style={{ whiteSpace: 'pre-wrap' }}>{message}</p> : null}
      {savedPath ? (
        <>
          <p
            style={{
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '0.8rem',
              wordBreak: 'break-all',
              background: 'var(--bg-secondary)',
              padding: '0.5rem 0.75rem',
              borderRadius: 6,
            }}
          >
            {savedPath}
          </p>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            {!isMobile ? (
              <button className="modal__primary" onClick={revealInFinder}>
                Klasörde göster
              </button>
            ) : null}
            <button className="modal__close" onClick={copyPath}>
              Yolu kopyala
            </button>
          </div>
        </>
      ) : null}
    </Modal>
  );
}
