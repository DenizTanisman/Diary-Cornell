import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

import { DEFAULT_LLM_SETTINGS, type LlmSettings } from '../../types/llmSettings';

type HealthState = 'unknown' | 'checking' | 'ok' | 'error';

export function LlmSettingsPanel() {
  const [settings, setSettings] = useState<LlmSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [health, setHealth] = useState<HealthState>('unknown');
  const [busy, setBusy] = useState(false);

  const reload = async () => {
    try {
      const s = await invoke<LlmSettings>('llm_get_settings');
      setSettings(s);
      if (s.enabled) {
        void checkHealth();
      } else {
        setHealth('unknown');
      }
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    reload();
  }, []);

  const checkHealth = async () => {
    setHealth('checking');
    try {
      const ok = await invoke<boolean>('llm_health');
      setHealth(ok ? 'ok' : 'error');
    } catch {
      setHealth('error');
    }
  };

  const save = async () => {
    if (!settings) return;
    setBusy(true);
    setError(null);
    try {
      await invoke('llm_save_settings', { settings });
      if (settings.enabled) {
        await checkHealth();
      }
      window.dispatchEvent(new CustomEvent('llm-settings-changed'));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!settings) {
    return (
      <div className="llm-settings">
        <h3 style={{ marginTop: 0 }}>LLM Features</h3>
        {error ? (
          <div role="alert" style={{ color: '#BA2222' }}>
            {error}
          </div>
        ) : (
          <p>Loading…</p>
        )}
      </div>
    );
  }

  const update = (patch: Partial<LlmSettings>) =>
    setSettings({ ...settings, ...patch });

  return (
    <div className="llm-settings" style={{ marginTop: '1.5rem' }}>
      <h3 style={{ marginTop: 0 }}>LLM Features</h3>

      {error && (
        <div role="alert" style={{ color: '#BA2222', marginBottom: '0.6rem' }}>
          {error}
        </div>
      )}

      <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
        <input
          type="checkbox"
          checked={settings.enabled}
          onChange={(e) => update({ enabled: e.target.checked })}
        />
        Enable LLM features (summarize, tag, sentiment)
      </label>

      {settings.enabled && (
        <fieldset
          style={{
            marginTop: '0.6rem',
            padding: '0.8rem 1rem',
            border: '1px solid rgba(0,0,0,0.15)',
            borderRadius: 8,
          }}
        >
          <label style={{ display: 'block', marginBottom: '0.6rem' }}>
            Bridge URL
            <input
              type="text"
              value={settings.bridgeUrl}
              onChange={(e) => update({ bridgeUrl: e.target.value })}
              placeholder="http://localhost:8765"
              style={{ display: 'block', width: '100%', marginTop: 4 }}
            />
          </label>
          <label style={{ display: 'block', marginBottom: '0.6rem' }}>
            API Key
            <input
              type="password"
              autoComplete="off"
              value={settings.bridgeApiKey ?? ''}
              onChange={(e) =>
                update({ bridgeApiKey: e.target.value === '' ? null : e.target.value })
              }
              style={{ display: 'block', width: '100%', marginTop: 4 }}
            />
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
            <input
              type="checkbox"
              checked={settings.autoSummarize}
              onChange={(e) => update({ autoSummarize: e.target.checked })}
            />
            Auto-summarize on save (extra LLM call per save)
          </label>
          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.5rem',
              marginTop: '0.4rem',
            }}
          >
            <input
              type="checkbox"
              checked={settings.autoTag}
              onChange={(e) => update({ autoTag: e.target.checked })}
            />
            Auto-tag on save
          </label>

          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.5rem',
              marginTop: '0.8rem',
              fontSize: '0.85rem',
            }}
          >
            <span>Bridge status:</span>
            <strong>
              {health === 'ok' && '✓ Connected'}
              {health === 'error' && '✗ Unreachable'}
              {health === 'checking' && '…'}
              {health === 'unknown' && '?'}
            </strong>
            <button onClick={checkHealth} disabled={busy || health === 'checking'}>
              Test connection
            </button>
          </div>
        </fieldset>
      )}

      <div style={{ marginTop: '0.8rem' }}>
        <button onClick={save} disabled={busy}>
          {busy ? 'Saving…' : 'Save'}
        </button>
        <button
          onClick={() => setSettings({ ...DEFAULT_LLM_SETTINGS })}
          disabled={busy}
          style={{ marginLeft: '0.5rem' }}
        >
          Reset
        </button>
      </div>
    </div>
  );
}
