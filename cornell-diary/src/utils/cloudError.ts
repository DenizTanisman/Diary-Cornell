/**
 * Tauri's `DomainError` serializes as `{ code, message }`. A naive
 * `String(e)` on the rejection produces "[object Object]" — useless to
 * a user trying to figure out what failed (e.g. profile insert).
 * `extractDomainErrorMessage` pulls .message and prefixes the variant
 * code so users see something like "[Validation] base_url is empty"
 * or "[Storage] cloud_profile sqlx: …" instead.
 */
export function extractDomainErrorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object') {
    const env = e as { code?: string; message?: string };
    if (env.message) return env.code ? `[${env.code}] ${env.message}` : env.message;
  }
  return 'unknown error';
}
