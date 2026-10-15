export type WubiPreferences = {
  wubi_mixed_pinyin?: boolean;
  wubi_code_hint?: boolean;
  /** 缺省为开：本次之前它没有开关，唯一可能的实际行为就是开。 */
  wubi_auto_commit_unique?: boolean;
};

/** 只投影控件状态；混输缺键时跟随版本，其余两项沿用旧行为，不补写偏好。 */
export function wubiControlState(
  preferences: WubiPreferences,
  mixedPinyinDefault = false,
): Required<WubiPreferences> {
  return {
    wubi_mixed_pinyin: preferences.wubi_mixed_pinyin ?? mixedPinyinDefault,
    wubi_code_hint: preferences.wubi_code_hint ?? true,
    wubi_auto_commit_unique: preferences.wubi_auto_commit_unique ?? true,
  };
}
