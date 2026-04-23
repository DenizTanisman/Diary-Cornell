import { useEffect, useRef, useState } from 'react';
import QRCode from 'qrcode';
import { useRepository } from '../../db/RepositoryContext';
import { exportToJSON, serializeExport } from '../../sync/exporter';
import { chunkJSONForQR, encodeFrame, type QRChunkFrame } from '../../sync/qrChunker';
import { getDeviceId } from '../../utils/deviceId';
import { QR_FRAME_INTERVAL_MS } from '../../constants/config';
import { Modal } from '../ui/Modal';
import { logger } from '../../utils/logger';

interface Props {
  onClose: () => void;
}

export function QRGenerator({ onClose }: Props) {
  const repo = useRepository();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [frames, setFrames] = useState<QRChunkFrame[]>([]);
  const [idx, setIdx] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const entries = await repo.getAll();
        const deviceId = await getDeviceId();
        const file = await exportToJSON(entries, deviceId);
        const serialized = await serializeExport(file);
        const produced = chunkJSONForQR(serialized);
        setFrames(produced);
      } catch (err) {
        logger.error('qr_generate_failed', { error: String(err) });
        setError(String(err));
      }
    })();
  }, [repo]);

  useEffect(() => {
    if (frames.length === 0) return;
    const id = setInterval(() => {
      setIdx((i) => (i + 1) % frames.length);
    }, QR_FRAME_INTERVAL_MS);
    return () => clearInterval(id);
  }, [frames.length]);

  useEffect(() => {
    if (!canvasRef.current || frames.length === 0) return;
    const frame = frames[idx];
    QRCode.toCanvas(canvasRef.current, encodeFrame(frame), { width: 320, margin: 1 }).catch(
      (err) => logger.error('qr_render_failed', { error: String(err) }),
    );
  }, [idx, frames]);

  return (
    <Modal
      title="QR Gönder"
      onClose={onClose}
      actions={
        <button className="modal__close" onClick={onClose}>
          Kapat
        </button>
      }
    >
      {error ? (
        <p className="app-error">{error}</p>
      ) : frames.length === 0 ? (
        <p>Hazırlanıyor…</p>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 }}>
          <canvas ref={canvasRef} />
          <span className="cornell-header__counter">
            Kare {idx + 1} / {frames.length}
          </span>
        </div>
      )}
    </Modal>
  );
}
