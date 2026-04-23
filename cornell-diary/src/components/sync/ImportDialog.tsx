import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { useRepository } from '../../db/RepositoryContext';
import { ImportError, importFromJSON } from '../../sync/importer';
import { Modal } from '../ui/Modal';
import { useT } from '../../locales';
import type { SyncResult } from '../../types/sync';
import { logger } from '../../utils/logger';

interface Props {
  onClose: () => void;
  onImported?: (result: SyncResult) => void;
}

export function ImportDialog({ onClose, onImported }: Props) {
  const t = useT();
  const repo = useRepository();
  const [status, setStatus] = useState<'idle' | 'working' | 'done' | 'error' | 'checksum'>('idle');
  const [message, setMessage] = useState<string | null>(null);
  const [pendingRaw, setPendingRaw] = useState<string | null>(null);

  const runImport = async (raw: string, ignoreChecksumMismatch: boolean) => {
    setStatus('working');
    try {
      const result = await importFromJSON(raw, repo, { ignoreChecksumMismatch });
      setStatus('done');
      setMessage(
        t('sync.result', {
          inserted: result.inserted,
          updated: result.updated,
          skipped: result.skipped,
        }),
      );
      onImported?.(result);
    } catch (err) {
      if (err instanceof ImportError && err.code === 'checksum_mismatch') {
        setStatus('checksum');
        setMessage(t('sync.checksumWarning'));
        return;
      }
      logger.error('import_failed', { error: String(err) });
      setStatus('error');
      setMessage(String(err));
    }
  };

  const handlePick = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (!selected || typeof selected !== 'string') return;
      const raw = await readTextFile(selected);
      setPendingRaw(raw);
      await runImport(raw, false);
    } catch (err) {
      logger.error('import_pick_failed', { error: String(err) });
      setStatus('error');
      setMessage(String(err));
    }
  };

  const handleForce = () => {
    if (!pendingRaw) return;
    void runImport(pendingRaw, true);
  };

  return (
    <Modal
      title={t('sync.importTitle')}
      onClose={onClose}
      actions={
        <>
          <button className="modal__close" onClick={onClose}>
            Kapat
          </button>
          {status === 'checksum' ? (
            <button className="modal__primary" onClick={handleForce}>
              Yine de içe aktar
            </button>
          ) : (
            <button
              className="modal__primary"
              onClick={handlePick}
              disabled={status === 'working'}
            >
              {status === 'working' ? '…' : t('sync.importAction')}
            </button>
          )}
        </>
      }
    >
      <p>{t('sync.importDescription')}</p>
      {message ? <p>{message}</p> : null}
    </Modal>
  );
}
