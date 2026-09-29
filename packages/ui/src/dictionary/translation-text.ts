/** The engine treats any non-ASCII source character as a Chinese-direction translation. */
export function isChineseTranslationSource(source: string): boolean {
  for (const character of source) {
    if ((character.codePointAt(0) ?? 0) > 0x7f) return true;
  }
  return false;
}
