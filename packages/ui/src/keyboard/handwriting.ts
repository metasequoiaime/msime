const MAX_HANDWRITING_CANDIDATES = 12;

function hasBmpCjk(text: string) {
  return Array.from(text).some((character) => {
    const code = character.codePointAt(0) ?? 0;
    return (
      (code >= 0x3400 && code <= 0x4dbf) ||
      (code >= 0x4e00 && code <= 0x9fff) ||
      (code >= 0xf900 && code <= 0xfaff)
    );
  });
}

export function normalizeHandwritingCandidates(candidates: string[]) {
  const seen = new Set<string>();
  const chinese: string[] = [];
  const other: string[] = [];
  for (const candidate of candidates) {
    if (!candidate || seen.has(candidate)) continue;
    seen.add(candidate);
    (hasBmpCjk(candidate) ? chinese : other).push(candidate);
  }
  return [...chinese, ...other].slice(0, MAX_HANDWRITING_CANDIDATES);
}
