import { useEffect, useRef, useState } from 'react';
import QrScanner from 'qr-scanner';
import {
  addFrame,
  assemble,
  createAssembler,
  isComplete,
  progress,
  type AssemblerState,
} from '../../sync/qrAssembler';
import { decodeFrame } from '../../sync/qrChunker';
import { importFromJSON } from '../../sync/importer';
import { useRepository } from '../../db/RepositoryContext';
import { Modal } from '../ui/Modal';
import { logger } from '../../utils/logger';
import type { SyncResult } from '../../types/sync';
import { useT } from '../../locales';

interface Props {
  onClose: () => void;
  onImported?: (result: SyncResult) => void;
}

export function QRScanner({ onClose, onImported }: Props) {
  const t = useT();
  const repo = useRepository();
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const assemblerRef = useRef<AssemblerState>(createAssembler());
  const scannerRef = useRef<QrScanner | null>(null);

  const repoRef = useRef(repo);
  const tRef = useRef(t);
  const onImportedRef = useRef(onImported);
  repoRef.current = repo;
  tRef.current = t;
  onImportedRef.current = onImported;

  const [status, setStatus] = useState('Kameraya erişiliyor…');
  const [count, setCount] = useState({ received: 0, total: 0 });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    let cancelled = false;

    const scanner = new QrScanner(
      video,
      async (result) => {
        const frame = decodeFrame(result.data);
        if (!frame) return;
        const accept = addFrame(assemblerRef.current, frame);
        if (!accept.accepted) return;
        setCount(progress(assemblerRef.current));
        if (!isComplete(assemblerRef.current)) return;
        try {
          scanner.stop();
          const raw = assemble(assemblerRef.current);
          setStatus('Kayıtlar birleştiriliyor…');
          const out = await importFromJSON(raw, repoRef.current);
          if (cancelled) return;
          setStatus(
            tRef.current('sync.result', {
              inserted: out.inserted,
              updated: out.updated,
              skipped: out.skipped,
            }),
          );
          onImportedRef.current?.(out);
        } catch (err) {
          logger.error('qr_scan_import_failed', { error: String(err) });
          setError(String(err));
        }
      },
      {
        highlightScanRegion: true,
        highlightCodeOutline: true,
        preferredCamera: 'environment',
      },
    );

    scannerRef.current = scanner;
    scanner
      .start()
      .then(() => !cancelled && setStatus('Kareleri tara…'))
      .catch((err) => {
        logger.error('qr_scan_start_failed', { error: String(err) });
        setError(`Kamera açılamadı: ${String(err)}`);
      });

    return () => {
      cancelled = true;
      scanner.stop();
      scanner.destroy();
      scannerRef.current = null;
    };
  }, []);

  return (
    <Modal
      title="QR Tara"
      onClose={onClose}
      actions={
        <button className="modal__close" onClick={onClose}>
          Kapat
        </button>
      }
    >
      {error ? (
        <p className="app-error">{error}</p>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 }}>
          <video
            ref={videoRef}
            playsInline
            muted
            autoPlay
            style={{
              width: '100%',
              maxWidth: 360,
              aspectRatio: '1 / 1',
              objectFit: 'cover',
              background: '#000',
              borderRadius: 8,
            }}
          />
          <p>{status}</p>
          {count.total > 0 ? (
            <span className="cornell-header__counter">
              {count.received}/{count.total} kare
            </span>
          ) : null}
        </div>
      )}
    </Modal>
  );
}
