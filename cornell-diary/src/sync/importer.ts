import type { IDiaryRepository } from '../db/IDiaryRepository';
import type { DiaryEntry } from '../types/diary';
import type { ExportFile, SyncResult } from '../types/sync';
import { exportFileSchema } from '../utils/validation';
import { sha256 } from '../utils/crypto';
import { canonicalizeEntries } from './exporter';
import { logger } from '../utils/logger';

export interface ImportOptions {
  ignoreChecksumMismatch?: boolean;
}

export class ImportError extends Error {
  constructor(message: string, public readonly code: string) {
    super(message);
    this.name = 'ImportError';
  }
}

export async function parseExportFile(raw: string): Promise<ExportFile> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new ImportError(`Geçersiz JSON: ${String(err)}`, 'invalid_json');
  }

  const result = exportFileSchema.safeParse(parsed);
  if (!result.success) {
    const msg = result.error.issues.map((i) => `${i.path.join('.')}: ${i.message}`).join('; ');
    throw new ImportError(`Geçersiz dışa aktarma formatı: ${msg}`, 'invalid_schema');
  }

  return result.data as ExportFile;
}

export async function verifyExportChecksum(file: ExportFile): Promise<boolean> {
  const actual = await sha256(canonicalizeEntries(file.entries));
  return actual === file.checksum;
}

export async function importFromJSON(
  raw: string,
  repository: IDiaryRepository,
  opts: ImportOptions = {},
): Promise<SyncResult> {
  const file = await parseExportFile(raw);

  const checksumOk = await verifyExportChecksum(file);
  if (!checksumOk && !opts.ignoreChecksumMismatch) {
    throw new ImportError(
      'Checksum uyuşmuyor — veri bozulmuş olabilir.',
      'checksum_mismatch',
    );
  }

  if (!checksumOk) {
    logger.warn('import_checksum_mismatch_ignored', { deviceId: file.deviceId });
  }

  const entries: DiaryEntry[] = file.entries.map((e) => ({
    date: e.date,
    diary: e.diary,
    cueItems: e.cueItems,
    summary: e.summary,
    quote: e.quote,
    createdAt: e.createdAt,
    updatedAt: e.updatedAt,
    deviceId: e.deviceId,
    version: e.version,
  }));

  try {
    const result = await repository.bulkUpsert(entries);
    return { ...result, errors: [] };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    logger.error('import_bulk_upsert_failed', { error: msg, count: entries.length });
    throw new ImportError(`Veritabanı yazımı başarısız: ${msg}`, 'db_write_failed');
  }
}
