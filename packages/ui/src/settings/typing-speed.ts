export type TypingBreakdown = {
  characters: Record<string, number>;
  sources: Record<string, number>;
};

export function withUnknown(
  value: Partial<TypingBreakdown> | undefined,
  total: number,
): TypingBreakdown {
  const characters = { ...value?.characters };
  const sourceCounts = { ...value?.sources };
  characters.unknown =
    (characters.unknown ?? 0) +
    Math.max(0, total - Object.values(characters).reduce((a, b) => a + b, 0));
  sourceCounts.unknown =
    (sourceCounts.unknown ?? 0) +
    Math.max(0, total - Object.values(sourceCounts).reduce((a, b) => a + b, 0));
  return { characters, sources: sourceCounts };
}

/**
 * Speed counts prose characters. Digits, punctuation, emoji, and symbols are excluded so short
 * bursts such as phone numbers do not appear as typing speed. `otherLetter` includes kana, hangul,
 * and other scripts used by the Japanese input mode.
 */
const speedCharacterKinds = ["han", "latin", "otherLetter"] as const;

/** Characters that count toward speed; zero for days with no recorded breakdown. */
export function readableCharacters(detail: Partial<TypingBreakdown> | undefined): number {
  return speedCharacterKinds.reduce((total, kind) => total + (detail?.characters?.[kind] ?? 0), 0);
}

/** Characters per active minute; zero when either side is missing. */
export function charactersPerMinute(characters: number, activeMs: number): number {
  if (characters <= 0 || activeMs <= 0) return 0;
  return characters / (activeMs / 60_000);
}
