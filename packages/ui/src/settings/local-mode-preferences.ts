export type LocalModeKey =
  | "unicode"
  | "date_time"
  | "date_time_inline"
  | "date_time_menu"
  | "quick_phrase"
  | "emoji"
  | "kaomoji"
  | "symbol"
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
  /** 日期时间混入普通候选（Windows `utility.date_time_candidates`，出厂关）：全拼输入 riqi/sj/xq/nongli 时把当前日期、时间、星期或农历接在对应的词后面。关着时不写入文档，缺键读作关。 */
  date_time_inline?: boolean;
  /** 「📅日期」入口：日期时间混入候选时第三位放入口，选中后候选窗换成全部格式。开着时不写入文档，缺键读作开。 */
  date_time_menu?: boolean;
  quick_phrase: boolean;
  emoji: boolean;
  kaomoji: boolean;
  /** 符号模式（Shift+S）：分组码与符号名两种查法。与 Windows 正式版的出厂配置一致，缺键读作开。 */
  symbol?: boolean;
  super_jianpin: boolean;
  temporary_english: boolean;
  temporary_japanese: boolean;
  /** V 模式（计算与数字）默认开，与 Windows `utility.v_mode` 出厂一致，缺键读作开；/ 和 @ 模式默认关，关闭时不写入文档。 */
  expression?: boolean;
  command?: boolean;
  mention?: boolean;
  /** @ 模式在用户自己的名字之后提供内置的全国省、市、区县；和 /、@ 一样，关闭时不写入文档。 */
  mention_places?: boolean;
  /** 「Shift+字母进入快捷模式」总开关。缺省读作开：早于它的文档里没有这个键。 */
  shift_letter_entry?: boolean;
  /** 拆字反查（C 模式）。缺键表示用户没动过开关，读作开，与 Windows `utility.radical_mode` 出厂一致。只有用户动过开关时才写进文档。 */
  chaizi?: boolean;
};

export const defaultLocalModes: LocalModePreferences = {
  unicode: true,
  date_time: true,
  quick_phrase: true,
  emoji: true,
  kaomoji: true,
  symbol: true,
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
  "symbol",
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
  // 缺键时的缺省沿用既有读法：符号模式、日期时间混输、日期格式入口读作开；拆字由宿主形态决定（显示总开关的桌面读关、不显示的触屏读开）；其余后加成员读作关。
  const defaultsOn =
    key === "symbol" ||
    key === "date_time_menu" ||
    key === "date_time_inline" ||
    (key === "chaizi" && !shiftLetterEntrySwitch);
  return {
    checked: preferences[key] ?? defaultsOn,
    disabled:
      (key === "mention_places" && !preferences.mention) ||
      (key === "date_time_menu" && !(preferences.date_time_inline ?? true)) ||
      (!shiftLetterEntry && shiftLetterModes.has(key)),
  };
}
