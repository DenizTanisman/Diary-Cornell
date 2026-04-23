export function sanitizeText(input: string, maxLength = 100_000): string {
  if (typeof input !== 'string') return '';
  return input.slice(0, maxLength);
}

export function sanitizeTitle(input: string): string {
  return sanitizeText(input, 200).replace(/\r?\n/g, ' ').trim();
}
