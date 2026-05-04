import { describe, expect, it } from 'vitest';

import {
  DEFAULT_LLM_SETTINGS,
  type LlmSettings,
  type SentimentResponse,
  type SummarizeResponse,
} from '../../src/types/llmSettings';

describe('DEFAULT_LLM_SETTINGS', () => {
  it('starts disabled with the bridge default URL', () => {
    expect(DEFAULT_LLM_SETTINGS.enabled).toBe(false);
    expect(DEFAULT_LLM_SETTINGS.bridgeUrl).toBe('http://localhost:8765');
    expect(DEFAULT_LLM_SETTINGS.bridgeApiKey).toBeNull();
    expect(DEFAULT_LLM_SETTINGS.preferredLanguage).toBe('auto');
  });

  it('matches the LlmSettings type without optional gaps', () => {
    const s: LlmSettings = DEFAULT_LLM_SETTINGS;
    // Smoke test: every field present.
    expect(Object.keys(s).sort()).toEqual(
      ['autoSummarize', 'autoTag', 'bridgeApiKey', 'bridgeUrl', 'enabled', 'preferredLanguage'],
    );
  });
});

describe('Bridge response shapes', () => {
  it('SummarizeResponse round-trips through JSON', () => {
    const r: SummarizeResponse = { summary: 'ok', model: 'mock-1', cached: false };
    expect(JSON.parse(JSON.stringify(r))).toEqual(r);
  });

  it('SentimentResponse label is constrained at the type level', () => {
    const r: SentimentResponse = {
      label: 'neutral',
      score: 0.5,
      summary: 'Mixed.',
      model: 'mock-1',
    };
    expect(['positive', 'neutral', 'negative']).toContain(r.label);
  });
});
