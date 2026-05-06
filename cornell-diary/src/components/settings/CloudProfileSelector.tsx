import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

import { isProtectedProfile, type CloudProfile } from '../../types/cloudProfile';

interface ProfileFormValues {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
}

const EMPTY_FORM: ProfileFormValues = {
  id: '',
  name: '',
  baseUrl: '',
  apiKey: '',
};

export function CloudProfileSelector() {
  const [profiles, setProfiles] = useState<CloudProfile[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [form, setForm] = useState<ProfileFormValues | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [restartHint, setRestartHint] = useState(false);

  const reload = async () => {
    setError(null);
    try {
      const list = await invoke<CloudProfile[]>('list_cloud_profiles');
      const active = await invoke<CloudProfile>('get_active_cloud_profile');
      setProfiles(list);
      setActiveId(active.id);
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    reload();
  }, []);

  const handleSwitch = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      await invoke('set_active_cloud_profile', { id });
      setActiveId(id);
      setRestartHint(true);
      window.dispatchEvent(new CustomEvent('cloud-profile-changed', { detail: id }));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const startEdit = (p: CloudProfile) => {
    setEditingId(p.id);
    setForm({
      id: p.id,
      name: p.name,
      baseUrl: p.baseUrl,
      apiKey: p.apiKey ?? '',
    });
  };

  const startCreate = () => {
    setEditingId(null);
    setForm({ ...EMPTY_FORM });
  };

  const saveForm = async () => {
    if (!form) return;
    if (!form.id.trim()) {
      setError('id boş olamaz');
      return;
    }
    if (!form.name.trim()) {
      setError('name boş olamaz');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await invoke('upsert_cloud_profile', {
        profile: {
          id: form.id.trim(),
          name: form.name.trim(),
          baseUrl: form.baseUrl.trim(),
          apiKey: form.apiKey.trim() === '' ? null : form.apiKey.trim(),
          isActive: false,
        },
      });
      setForm(null);
      setEditingId(null);
      await reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const deleteProfile = async (id: string) => {
    if (isProtectedProfile(id)) return;
    if (!confirm(`Delete profile "${id}"?`)) return;
    setBusy(true);
    setError(null);
    try {
      await invoke('delete_cloud_profile', { id });
      await reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="cloud-profile-selector">
      <h3 style={{ marginTop: 0 }}>Cloud Profile</h3>

      {error && (
        <div role="alert" style={{ color: '#BA2222', marginBottom: '0.6rem' }}>
          {error}
        </div>
      )}

      {restartHint && (
        <div
          role="status"
          style={{
            marginBottom: '0.8rem',
            padding: '0.5rem 0.7rem',
            borderRadius: 6,
            background: 'rgba(186, 117, 23, 0.12)',
            color: '#7a4d09',
            fontSize: '0.85rem',
          }}
        >
          Profil değişti. Yeni URL'ye geçmek için Diary'yi yeniden başlat.
        </div>
      )}

      <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
        {profiles.map((p) => (
          <li
            key={p.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.6rem',
              padding: '0.5rem 0',
              borderBottom: '1px solid rgba(0,0,0,0.08)',
            }}
          >
            <input
              type="radio"
              name="cloud-profile"
              checked={activeId === p.id}
              disabled={busy}
              onChange={() => handleSwitch(p.id)}
              aria-label={`Activate ${p.name}`}
            />
            <span style={{ flex: 1 }}>
              <strong>{p.name}</strong>
              <span style={{ display: 'block', fontSize: '0.78rem', opacity: 0.7 }}>
                {p.baseUrl || '(not configured)'}
              </span>
            </span>
            <button onClick={() => startEdit(p)} disabled={busy}>
              Edit
            </button>
            {!isProtectedProfile(p.id) && (
              <button onClick={() => deleteProfile(p.id)} disabled={busy}>
                Delete
              </button>
            )}
          </li>
        ))}
      </ul>

      <div style={{ marginTop: '0.8rem' }}>
        <button onClick={startCreate} disabled={busy || form !== null}>
          + Add Custom Profile
        </button>
      </div>

      {form && (
        <fieldset
          style={{
            marginTop: '1rem',
            padding: '0.8rem 1rem',
            border: '1px solid rgba(0,0,0,0.15)',
            borderRadius: 8,
          }}
        >
          <legend>{editingId ? `Edit ${editingId}` : 'New profile'}</legend>
          <label style={{ display: 'block', marginBottom: '0.5rem' }}>
            ID
            <input
              type="text"
              value={form.id}
              disabled={editingId !== null}
              onChange={(e) => setForm({ ...form, id: e.target.value })}
              placeholder="my-staging"
              style={{ display: 'block', width: '100%', marginTop: 4 }}
            />
          </label>
          <label style={{ display: 'block', marginBottom: '0.5rem' }}>
            Name
            <input
              type="text"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="Staging"
              style={{ display: 'block', width: '100%', marginTop: 4 }}
            />
          </label>
          <label style={{ display: 'block', marginBottom: '0.5rem' }}>
            Base URL
            <input
              type="text"
              value={form.baseUrl}
              onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
              placeholder="https://cloud.example.com"
              style={{ display: 'block', width: '100%', marginTop: 4 }}
            />
          </label>
          <label style={{ display: 'block', marginBottom: '0.5rem' }}>
            API key (optional)
            <input
              type="password"
              value={form.apiKey}
              autoComplete="off"
              onChange={(e) => setForm({ ...form, apiKey: e.target.value })}
              style={{ display: 'block', width: '100%', marginTop: 4 }}
            />
          </label>
          <div style={{ display: 'flex', gap: '0.5rem', marginTop: '0.5rem' }}>
            <button onClick={saveForm} disabled={busy}>
              {busy ? 'Saving…' : 'Save'}
            </button>
            <button
              onClick={() => {
                setForm(null);
                setEditingId(null);
              }}
              disabled={busy}
            >
              Cancel
            </button>
          </div>
        </fieldset>
      )}
    </div>
  );
}
