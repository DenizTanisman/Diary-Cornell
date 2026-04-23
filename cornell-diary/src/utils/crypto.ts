export async function sha256(data: string): Promise<string> {
  const encoder = new TextEncoder();
  const dataBuffer = encoder.encode(data);
  const hashBuffer = await crypto.subtle.digest('SHA-256', dataBuffer);
  return (
    'sha256:' +
    Array.from(new Uint8Array(hashBuffer))
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('')
  );
}

export async function verifyChecksum(data: string, expected: string): Promise<boolean> {
  const actual = await sha256(data);
  return actual === expected;
}
