/** Return a host error's optional machine-readable code without exposing its payload. */
export function errorCode(error: unknown): string | undefined {
  if (typeof error !== "object" || error === null || !("code" in error)) return undefined;
  const code = (error as { code: unknown }).code;
  return code == null ? undefined : String(code);
}
