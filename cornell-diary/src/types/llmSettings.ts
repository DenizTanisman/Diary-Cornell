/**
 * Wire shapes for the LLM Bridge IPC. Mirror the Rust DTOs in
 * `src-tauri/src/db/llm_settings.rs` and `src-tauri/src/sync/bridge_client.rs`.
 */
export interface LlmSettings {
  enabled: boolean;
  bridgeUrl: string;
  bridgeApiKey: string | null;
  autoSummarize: boolean;
  autoTag: boolean;
  preferredLanguage: string;
}

export const DEFAULT_LLM_SETTINGS: LlmSettings = {
  enabled: false,
  bridgeUrl: 'http://localhost:8765',
  bridgeApiKey: null,
  autoSummarize: false,
  autoTag: false,
  preferredLanguage: 'auto',
};

export type SummaryStyle = 'brief' | 'detailed' | 'bullet';

export interface SummarizeResponse {
  summary: string;
  model: string;
  cached: boolean;
}

export interface TagResponse {
  tags: string[];
  model: string;
}

export interface SentimentResponse {
  label: 'positive' | 'neutral' | 'negative';
  score: number;
  summary: string;
  model: string;
}
