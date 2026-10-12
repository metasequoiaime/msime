/** Keep a finite-range value within its inclusive bounds. */
export function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

/** Clamp a possibly invalid number, substituting a fallback for non-finite input. */
export function clampFinite(
  value: number,
  minimum: number,
  maximum: number,
  fallback: number,
): number {
  return Number.isFinite(value) ? clamp(value, minimum, maximum) : fallback;
}
