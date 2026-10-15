export type WordCharacterPreferences = {
  enabled: boolean;
  keys: "brackets" | "minus_equal";
};

export type NavigationPreferences = {
  minus_equal: boolean;
  comma_period: boolean;
  brackets: boolean;
  tab: boolean;
  page_up_down: boolean;
  mouse_wheel?: boolean;
  arrows: boolean;
};

export const defaultWordCharacter: WordCharacterPreferences = { enabled: true, keys: "brackets" };

export const defaultNavigation: NavigationPreferences = {
  minus_equal: true,
  comma_period: true,
  brackets: false,
  tab: true,
  page_up_down: true,
  arrows: true,
};

export type CandidatePagingChange = {
  navigation: NavigationPreferences;
  wordCharacter: WordCharacterPreferences;
};

/** 只有本次打开的翻页键占用同组按键时才让出以词定字，其他变更保留原对象。 */
function wordCharacterAfterPaging(
  wordCharacter: WordCharacterPreferences,
  key: keyof NavigationPreferences,
  enabled: boolean,
): WordCharacterPreferences {
  return enabled && wordCharacter.enabled && wordCharacter.keys === key
    ? { ...wordCharacter, enabled: false }
    : wordCharacter;
}

export function pagingShortcutChange(
  navigation: NavigationPreferences,
  wordCharacter: WordCharacterPreferences,
  key: keyof NavigationPreferences,
  enabled: boolean,
): CandidatePagingChange {
  return {
    navigation: { ...navigation, [key]: enabled },
    wordCharacter: wordCharacterAfterPaging(wordCharacter, key, enabled),
  };
}

export function restorePagingChange(
  wordCharacter: WordCharacterPreferences,
): CandidatePagingChange {
  return {
    navigation: defaultNavigation,
    wordCharacter: wordCharacterAfterPaging(
      wordCharacter,
      wordCharacter.keys,
      defaultNavigation[wordCharacter.keys],
    ),
  };
}

export function wordCharacterEnabledChange(
  wordCharacter: WordCharacterPreferences,
  navigation: NavigationPreferences,
  enabled: boolean,
): CandidatePagingChange {
  return {
    wordCharacter: { ...wordCharacter, enabled },
    navigation: enabled ? { ...navigation, [wordCharacter.keys]: false } : navigation,
  };
}

export function pagingPreferencesPatch(
  wordCharacter: WordCharacterPreferences,
  next: CandidatePagingChange,
): { navigation: NavigationPreferences; word_character?: WordCharacterPreferences } {
  return {
    // 未发生冲突时不把原本缺省的以词定字偏好写入文档；保持现有的引用判据。
    ...(next.wordCharacter !== wordCharacter ? { word_character: next.wordCharacter } : {}),
    navigation: next.navigation,
  };
}
