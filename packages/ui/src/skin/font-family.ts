import { decodeCssUrl } from "./css-image-value";

/** Normalizes a CSS font-family token for case-insensitive lookup. */
export function fontFamilyKey(value: string): string | null {
  const quoted = value.startsWith('"') || value.startsWith("'");
  return decodeCssUrl(quoted ? value.slice(1, -1) : value, quoted)?.toLowerCase() ?? null;
}
