export type TypingBreakdown = {
  characters: Record<string, number>;
  sources: Record<string, number>;
};

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
