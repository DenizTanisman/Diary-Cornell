import '@testing-library/jest-dom/vitest';

if (!globalThis.crypto || !globalThis.crypto.subtle) {
  // jsdom >=24 ships with webcrypto, but be defensive
  const { webcrypto } = await import('node:crypto');
  Object.defineProperty(globalThis, 'crypto', { value: webcrypto, configurable: true });
}
