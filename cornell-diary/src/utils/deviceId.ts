import { nanoid } from 'nanoid';

const STORAGE_KEY = 'cornell-diary:deviceId';

let cached: string | null = null;

export async function getDeviceId(): Promise<string> {
  if (cached) return cached;

  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored) {
    cached = stored;
    return stored;
  }

  const prefix = await resolveDevicePrefix();
  const id = `${prefix}-${nanoid(8)}`;
  localStorage.setItem(STORAGE_KEY, id);
  cached = id;
  return id;
}

async function resolveDevicePrefix(): Promise<string> {
  try {
    const os = await import('@tauri-apps/plugin-os');
    const hostname = await os.hostname();
    if (hostname) return hostname.toLowerCase().replace(/[^a-z0-9-]/g, '-');
  } catch {
    // not running in tauri context (e.g. unit tests)
  }
  return 'device';
}
