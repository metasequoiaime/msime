export type LocalModeKey =
  | "unicode"
  | "date_time"
  | "quick_phrase"
  | "emoji"
  | "kaomoji"
  | "super_jianpin"
  | "temporary_english"
  | "temporary_japanese"
  | "chaizi"
  | "expression"
  | "command"
  | "mention"
  | "mention_places";

export type LocalModePreferences = {
  unicode: boolean;
  date_time: boolean;
  quick_phrase: boolean;
  emoji: boolean;
  kaomoji: boolean;
  super_jianpin: boolean;
  temporary_english: boolean;
  temporary_japanese: boolean;
  /** V、/ 和 @ 模式在新偏好中默认关闭，关闭时不写入文档，因此缺键读作关。 */
  expression?: boolean;
  command?: boolean;
  mention?: boolean;
  /** @ 模式在用户自己的名字之后提供内置的全国省、市、区县；和上面三项一样，关闭时不写入文档。 */
  mention_places?: boolean;
  /** 「Shift+字母进入快捷模式」总开关。和上面几项相反，缺省读作开：早于它的文档里没有这个键。 */
  shift_letter_entry?: boolean;
  /** 拆字反查（C 模式）。缺键表示用户没动过开关，由宿主形态决定：触屏宿主读作开，桌面宿主读作关。只有用户动过开关时才写进文档。 */
  chaizi?: boolean;
};

export const defaultLocalModes: LocalModePreferences = {
  unicode: true,
  date_time: true,
  quick_phrase: true,
  emoji: true,
  kaomoji: true,
  super_jianpin: true,
  temporary_english: true,
  temporary_japanese: true,
};

/** 从 Shift+字母进入的模式：总开关关掉时它们的开关保留原值，只是暂时不起作用。`/` 和 `@` 不在其中。 */
const shiftLetterModes: ReadonlySet<LocalModeKey> = new Set<LocalModeKey>([
  "unicode",
  "date_time",
  "quick_phrase",
  "emoji",
  "kaomoji",
  "super_jianpin",
  "temporary_english",
  "temporary_japanese",
  "chaizi",
  "expression",
]);

/** 只计算勾选和禁用状态，不修改偏好，也不补写尚未设置的缺省字段。 */
export function localModeControlState(
  preferences: LocalModePreferences,
  key: LocalModeKey,
  shiftLetterEntrySwitch: boolean,
) {
  // 只有显示总开关的宿主才按它禁用各行；其他宿主的入口保持可用。
  const shiftLetterEntry = !shiftLetterEntrySwitch || (preferences.shift_letter_entry ?? true);
  return {
    checked: preferences[key] ?? (key === "chaizi" && !shiftLetterEntrySwitch),
    disabled:
      (key === "mention_places" && !preferences.mention) ||
      (!shiftLetterEntry && shiftLetterModes.has(key)),
  };
}
