export const MAX_CUE_ITEMS = 7 as const;

export interface CueItem {
  position: number;
  title: string;
  content: string;
}

export interface DiaryEntry {
  date: string;
  diary: string;
  cueItems: CueItem[];
  summary: string;
  quote: string;
  createdAt: string;
  updatedAt: string;
  deviceId?: string;
  version: number;
}

export interface DbDiaryRow {
  date: string;
  diary: string;
  title_1: string | null;
  content_1: string | null;
  title_2: string | null;
  content_2: string | null;
  title_3: string | null;
  content_3: string | null;
  title_4: string | null;
  content_4: string | null;
  title_5: string | null;
  content_5: string | null;
  title_6: string | null;
  content_6: string | null;
  title_7: string | null;
  content_7: string | null;
  summary: string;
  quote: string;
  created_at: string;
  updated_at: string;
  device_id: string | null;
  version: number;
}

export function createEmptyEntry(date: string, deviceId: string): DiaryEntry {
  const now = new Date().toISOString();
  return {
    date,
    diary: '',
    cueItems: [],
    summary: '',
    quote: '',
    createdAt: now,
    updatedAt: now,
    deviceId,
    version: 1,
  };
}
