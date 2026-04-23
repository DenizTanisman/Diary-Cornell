import { z } from 'zod';
import { MAX_CUE_ITEMS } from '../types/diary';

export const cueItemSchema = z.object({
  position: z.number().int().min(1).max(MAX_CUE_ITEMS),
  title: z.string().max(200),
  content: z.string().max(50_000),
});

export const diaryEntrySchema = z.object({
  date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'date must be YYYY-MM-DD'),
  diary: z.string().max(200_000),
  cueItems: z.array(cueItemSchema).max(MAX_CUE_ITEMS),
  summary: z.string().max(2000),
  quote: z.string().max(2000),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
  deviceId: z.string().optional(),
  version: z.number().int().min(1),
});

export const exportFileSchema = z.object({
  $schema: z.string(),
  format: z.literal('cornell-diary-export'),
  version: z.string(),
  exportedAt: z.string(),
  deviceId: z.string(),
  entryCount: z.number().int().min(0),
  checksum: z.string().regex(/^sha256:[a-f0-9]{64}$/),
  entries: z.array(diaryEntrySchema),
});

export type ExportFileParsed = z.infer<typeof exportFileSchema>;
