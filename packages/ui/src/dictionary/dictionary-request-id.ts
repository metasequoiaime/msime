/** Create a unique request identifier for dictionary host operations. */
export function dictionaryRequestId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}
