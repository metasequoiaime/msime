export type FuzzyPinyinPreferences = { enabled: boolean; rules: string[]; seeded?: boolean };

export const defaultFuzzyPinyin: FuzzyPinyinPreferences = { enabled: false, rules: [] };

export const fuzzyPinyinGroups: [string, [string, string][]][] = [
  [
    "平翘舌",
    [
      ["z-zh", "z ↔ zh"],
      ["c-ch", "c ↔ ch"],
      ["s-sh", "s ↔ sh"],
    ],
  ],
  [
    "声母",
    [
      ["n-l", "n ↔ l"],
      ["f-h", "f ↔ h"],
      ["r-l", "r ↔ l"],
    ],
  ],
  [
    "前后鼻音",
    [
      ["an-ang", "an ↔ ang"],
      ["en-eng", "en ↔ eng"],
      ["in-ing", "in ↔ ing"],
    ],
  ],
  [
    "其他韵母",
    [
      ["ian-iang", "ian ↔ iang"],
      ["uan-uang", "uan ↔ uang"],
    ],
  ],
];

const fuzzyPinyinRuleIds = fuzzyPinyinGroups.flatMap(([, rules]) => rules.map(([id]) => id));

export function fuzzyPinyinEnabledChange(
  preferences: FuzzyPinyinPreferences,
  enabled: boolean,
): FuzzyPinyinPreferences {
  const firstEnable = enabled && !preferences.seeded;
  return {
    ...preferences,
    enabled,
    ...(firstEnable ? { rules: fuzzyPinyinRuleIds, seeded: true } : {}),
  };
}

export function fuzzyPinyinRuleChange(
  preferences: FuzzyPinyinPreferences,
  id: string,
  checked: boolean,
): FuzzyPinyinPreferences {
  const selected = new Set(preferences.rules);
  if (checked) selected.add(id);
  else selected.delete(id);
  return { ...preferences, rules: [...selected].sort() };
}

export function resetFuzzyPinyin(preferences: FuzzyPinyinPreferences): FuzzyPinyinPreferences {
  return { enabled: false, rules: [], seeded: preferences.seeded ?? false };
}
