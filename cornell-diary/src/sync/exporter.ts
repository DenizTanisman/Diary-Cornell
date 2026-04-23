import type { DiaryEntry } from '../types/diary';
import type { ExportFile } from '../types/sync';
import { sha256 } from '../utils/crypto';
import { EXPORT_FORMAT_VERSION, EXPORT_SCHEMA_URL } from '../constants/config';

function canonicalize(entries: DiaryEntry[]): string {
  const sorted = [...entries].sort((a, b) => a.date.localeCompare(b.date));
  const normalized = sorted.map((e) => ({
    date: e.date,
    diary: e.diary,
    cueItems: [...e.cueItems].sort((a, b) => a.position - b.position),
    summary: e.summary,
    quote: e.quote,
    createdAt: e.createdAt,
    updatedAt: e.updatedAt,
    version: e.version,
  }));
  return JSON.stringify(normalized);
}

export async function exportToJSON(
  entries: DiaryEntry[],
  deviceId: string,
): Promise<ExportFile> {
  const checksum = await sha256(canonicalize(entries));
  return {
    $schema: EXPORT_SCHEMA_URL,
    format: 'cornell-diary-export',
    version: EXPORT_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    deviceId,
    entryCount: entries.length,
    checksum,
    entries,
  };
}

export async function serializeExport(file: ExportFile): Promise<string> {
  return JSON.stringify(file, null, 2);
}

export { canonicalize as canonicalizeEntries };
