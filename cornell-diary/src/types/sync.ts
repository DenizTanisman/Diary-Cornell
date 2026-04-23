import type { DiaryEntry } from './diary';

export interface ExportFile {
  $schema: string;
  format: 'cornell-diary-export';
  version: string;
  exportedAt: string;
  deviceId: string;
  entryCount: number;
  checksum: string;
  entries: DiaryEntry[];
}

export interface SyncResult {
  inserted: number;
  updated: number;
  skipped: number;
  errors: string[];
}

export interface QRFrame {
  sessionId: string;
  frameNum: number;
  totalFrames: number;
  data: string;
}

export type SyncMethod = 'qr' | 'json_file';
export type SyncType = 'export' | 'import';
export type SyncStatus = 'success' | 'partial' | 'failed';
