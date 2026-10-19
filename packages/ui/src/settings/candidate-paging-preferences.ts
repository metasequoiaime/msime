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
  /** 左右键在输入串两端时移动候选项；缺少该字段时按默认开启，与 `arrows` 各自独立。 */
  left_right?: boolean;
};

export const defaultWordCharacter: WordCharacterPreferences = { enabled: true, keys: "brackets" };

export const defaultNavigation: NavigationPreferences = {
  minus_equal: true,
  comma_period: true,
  brackets: false,
  tab: true,
  page_up_down: true,
  arrows: true,
  left_right: true,
};

/** 读取旧文档时补齐新增的导航偏好；可选的鼠标滚轮字段仍保持缺省不写入。 */
export function navigationWithDefaults(navigation?: NavigationPreferences): NavigationPreferences {
  return navigation
    ? {
        ...defaultNavigation,
        ...navigation,
        left_right: navigation.left_right ?? defaultNavigation.left_right,
      }
    : defaultNavigation;
}

/** 宿主把 Tab、PageUp/PageDown 两组翻页键固定为开时（`HostCapabilities.tab_page_paging_switches` 为假：macOS、Windows、HarmonyOS），速查按实际生效的样子列出它们，不看文档里存的值。 */
export function effectiveNavigation(
  navigation: NavigationPreferences,
  tabPageSwitches: boolean,
): NavigationPreferences {
  return tabPageSwitches ? navigation : { ...navigation, tab: true, page_up_down: true };
}

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

/** 合并后的「以词定字」单选的取值：关闭，或用哪组键。 */
export type WordCharacterChoice = "off" | WordCharacterPreferences["keys"];

export function wordCharacterChoice(wordCharacter: WordCharacterPreferences): WordCharacterChoice {
  return wordCharacter.enabled ? wordCharacter.keys : "off";
}

/** 选「关闭」只关掉以词定字，记着的键组不动，下次打开还是它；选一组键就打开并换成这组键。被翻页占用的键组在控件里不能选，所以这里不改翻页偏好。 */
export function wordCharacterChoiceChange(
  wordCharacter: WordCharacterPreferences,
  navigation: NavigationPreferences,
  choice: WordCharacterChoice,
): CandidatePagingChange {
  return {
    wordCharacter:
      choice === "off" ? { ...wordCharacter, enabled: false } : { enabled: true, keys: choice },
    navigation,
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
