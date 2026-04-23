import { nanoid } from 'nanoid';
import { QR_CHUNK_SIZE } from '../constants/config';

export const QR_PROTOCOL = 'CDIA1';

export interface QRChunkFrame {
  sessionId: string;
  frameNum: number;
  totalFrames: number;
  payload: string;
}

export function chunkJSONForQR(json: string, chunkSize = QR_CHUNK_SIZE): QRChunkFrame[] {
  if (!json) return [];
  const base64 = btoa(unescape(encodeURIComponent(json)));
  const sessionId = nanoid(8);
  const total = Math.ceil(base64.length / chunkSize);
  const frames: QRChunkFrame[] = [];
  for (let i = 0; i < total; i++) {
    frames.push({
      sessionId,
      frameNum: i + 1,
      totalFrames: total,
      payload: base64.slice(i * chunkSize, (i + 1) * chunkSize),
    });
  }
  return frames;
}

export function encodeFrame(frame: QRChunkFrame): string {
  return `${QR_PROTOCOL}|${frame.frameNum}/${frame.totalFrames}|${frame.sessionId}|${frame.payload}`;
}

export function decodeFrame(raw: string): QRChunkFrame | null {
  if (!raw.startsWith(QR_PROTOCOL + '|')) return null;
  const parts = raw.split('|');
  if (parts.length < 4) return null;
  const [, frameStr, sessionId, ...rest] = parts;
  const [numStr, totalStr] = frameStr.split('/');
  const frameNum = Number(numStr);
  const totalFrames = Number(totalStr);
  if (!Number.isFinite(frameNum) || !Number.isFinite(totalFrames)) return null;
  return {
    sessionId,
    frameNum,
    totalFrames,
    payload: rest.join('|'),
  };
}
