import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';

import type {
  SentimentResponse,
  SummarizeResponse,
  SummaryStyle,
  TagResponse,
} from '../../types/llmSettings';

interface Props {
  /** Concatenated entry text (cue items + notes + summary). Bridge will
   *  reject anything shorter than 10 chars or longer than 50K. */
  text: string;
}

export function LlmInsightsPanel({ text }: Props) {
  const [summary, setSummary] = useState<SummarizeResponse | null>(null);
  const [tags, setTags] = useState<string[] | null>(null);
  const [sentiment, setSentiment] = useState<SentimentResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState({
    summary: false,
    tags: false,
    sentiment: false,
  });

  const tooShort = text.trim().length < 20;

  const handleSummary = async (style: SummaryStyle) => {
    if (tooShort) {
      setError('Bu kayıt özet için fazla kısa.');
      return;
    }
    setLoading((p) => ({ ...p, summary: true }));
    setError(null);
    try {
      const r = await invoke<SummarizeResponse>('llm_summarize', {
        text,
        style,
        language: 'auto',
      });
      setSummary(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading((p) => ({ ...p, summary: false }));
    }
  };

  const handleTags = async () => {
    if (tooShort) {
      setError('Bu kayıt etiket için fazla kısa.');
      return;
    }
    setLoading((p) => ({ ...p, tags: true }));
    setError(null);
    try {
      const r = await invoke<TagResponse>('llm_tag', { text, maxTags: 5 });
      setTags(r.tags);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading((p) => ({ ...p, tags: false }));
    }
  };

  const handleSentiment = async () => {
    if (tooShort) {
      setError('Bu kayıt duygu analizi için fazla kısa.');
      return;
    }
    setLoading((p) => ({ ...p, sentiment: true }));
    setError(null);
    try {
      const r = await invoke<SentimentResponse>('llm_sentiment', { text });
      setSentiment(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading((p) => ({ ...p, sentiment: false }));
    }
  };

  return (
    <aside
      className="llm-insights"
      style={{
        padding: '0.8rem 1rem',
        border: '1px solid rgba(0,0,0,0.1)',
        borderRadius: 8,
      }}
    >
      <h4 style={{ marginTop: 0 }}>AI Insights</h4>

      {error && (
        <div role="alert" style={{ color: '#BA2222', marginBottom: '0.5rem' }}>
          {error}
        </div>
      )}

      <section style={{ marginBottom: '1rem' }}>
        <div style={{ display: 'flex', gap: '0.4rem', flexWrap: 'wrap' }}>
          <button onClick={() => handleSummary('brief')} disabled={loading.summary}>
            {loading.summary ? '…' : 'Brief'}
          </button>
          <button onClick={() => handleSummary('detailed')} disabled={loading.summary}>
            Detailed
          </button>
          <button onClick={() => handleSummary('bullet')} disabled={loading.summary}>
            Bullets
          </button>
        </div>
        {summary && (
          <div style={{ marginTop: '0.5rem' }}>
            <p style={{ margin: 0 }}>{summary.summary}</p>
            <small style={{ opacity: 0.6 }}>
              {summary.model}
              {summary.cached ? ' · cached' : ''}
            </small>
          </div>
        )}
      </section>

      <section style={{ marginBottom: '1rem' }}>
        <button onClick={handleTags} disabled={loading.tags}>
          {loading.tags ? 'Generating tags…' : 'Generate tags'}
        </button>
        {tags && tags.length > 0 && (
          <div style={{ marginTop: '0.4rem', display: 'flex', gap: '0.3rem', flexWrap: 'wrap' }}>
            {tags.map((t) => (
              <span
                key={t}
                style={{
                  padding: '0.15rem 0.5rem',
                  borderRadius: 999,
                  background: 'rgba(0,0,0,0.06)',
                  fontSize: '0.78rem',
                }}
              >
                {t}
              </span>
            ))}
          </div>
        )}
        {tags && tags.length === 0 && (
          <p style={{ marginTop: '0.4rem', fontSize: '0.85rem', opacity: 0.7 }}>
            Model boş etiket listesi döndü.
          </p>
        )}
      </section>

      <section>
        <button onClick={handleSentiment} disabled={loading.sentiment}>
          {loading.sentiment ? 'Analyzing…' : 'Analyze sentiment'}
        </button>
        {sentiment && (
          <div style={{ marginTop: '0.4rem' }}>
            <strong style={{ textTransform: 'capitalize' }}>{sentiment.label}</strong>{' '}
            <span style={{ opacity: 0.7 }}>({(sentiment.score * 100).toFixed(0)}%)</span>
            <p style={{ marginTop: 4 }}>{sentiment.summary}</p>
          </div>
        )}
      </section>
    </aside>
  );
}
