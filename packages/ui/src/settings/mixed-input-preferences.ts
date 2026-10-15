export type MixedInputPreferences = {
  english: boolean;
  minimum_prefix: number;
  emoji: boolean;
  kaomoji: boolean;
  /** 日文混输（实验）。后加的可选成员：Rust 端关着时不写这个键，旧文档和只写前四项的宿主都没有它，缺省按关处理；也因此不放进 `defaultMixedInput`。 */
  japanese?: boolean;
  /** 符号混输（打拼音出符号）。后加的可选成员：Rust 端开着时不写这个键，旧文档和只写前四项的宿主都没有它，缺省按开处理；也因此不放进 `defaultMixedInput`。 */
  symbols?: boolean;
};

export const defaultMixedInput: MixedInputPreferences = {
  english: true,
  minimum_prefix: 5,
  emoji: false,
  kaomoji: false,
};

/** 仅投影控件状态，不补写偏好或改变英文触发字符数。 */
export function mixedInputControlState(preferences: MixedInputPreferences) {
  return {
    symbolsChecked: preferences.symbols ?? true,
    japaneseChecked: preferences.japanese ?? false,
    minimumPrefixDisabled: !preferences.english,
  };
}
