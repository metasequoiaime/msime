export type WubiProfile = "wubi86" | "wubi98";

export type WubiCandidateSelectionPreferences = {
  enabled: boolean;
  second_key: string;
  third_key: string;
};

export const defaultWubiCandidateSelection: WubiCandidateSelectionPreferences = {
  enabled: false,
  second_key: ";",
  third_key: "'",
};

/** 与共享偏好校验一致：字母、数字、空白和重复字符都不能作快捷选词键。 */
export function validWubiCandidateKeys(second: string, third: string): boolean {
  const punctuation = /^[!-/:-@[-`{-~]$/;
  return punctuation.test(second) && punctuation.test(third) && second !== third;
}

export type WubiPreferences = {
  wubi_candidate_selection?: WubiCandidateSelectionPreferences;
  wubi_mixed_pinyin?: boolean;
  wubi_code_hint?: boolean;
  /** 缺省为开：本次之前它没有开关，唯一可能的实际行为就是开。 */
  wubi_auto_commit_unique?: boolean;
  /** 缺省为关，与来源出厂 `wubi_z_mode = "off"` 一致。 */
  wubi_z_wildcard?: boolean;
  /** 缺省为开，与来源出厂一致，也是接入这个开关之前唯一可能的行为。 */
  wubi_fifth_code_top_commit?: boolean;
};

/** 只投影控件状态；混输缺键时跟随版本，其余四项沿用各自的出厂行为，不补写偏好。 */
export function wubiControlState(
  preferences: WubiPreferences,
  mixedPinyinDefault = false,
): Required<Omit<WubiPreferences, "wubi_candidate_selection">> {
  return {
    wubi_mixed_pinyin: preferences.wubi_mixed_pinyin ?? mixedPinyinDefault,
    wubi_code_hint: preferences.wubi_code_hint ?? true,
    wubi_auto_commit_unique: preferences.wubi_auto_commit_unique ?? true,
    wubi_z_wildcard: preferences.wubi_z_wildcard ?? false,
    wubi_fifth_code_top_commit: preferences.wubi_fifth_code_top_commit ?? true,
  };
}
