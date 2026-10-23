import type {
  PunctuationMode,
  PunctuationPreferences,
  QuoteStyle,
  BracketStyle,
} from "./punctuation-preferences";

export type PunctuationSwitch =
  | "smart_punctuation"
  | "smart_punctuation_repeat"
  | "smart_punctuation_space_convert"
  | "smart_punctuation_direct_digit"
  | "smart_punctuation_direct_letter"
  | "paired_punctuation"
  | "caps_lock_ascii_punctuation"
  | "enumeration_comma_backslash"
  | "enumeration_comma_slash";

export type PunctuationSwitchCopy = readonly [PunctuationSwitch, string, string];

export const smartPunctuation: PunctuationSwitchCopy = [
  "smart_punctuation",
  "智能标点",
  "中文标点模式下，字母或数字后的 , . : 自动使用英文标点",
];

/** 「智能标点」的细分选项，紧接在它之后列出。 */
export const smartPunctuationRefinements: readonly PunctuationSwitchCopy[] = [
  [
    "smart_punctuation_repeat",
    "重复标点转中文",
    "智能标点输出英文标点后，2 秒内再次输入同一标点时替换为中文标点",
  ],
  [
    "smart_punctuation_space_convert",
    "中文标点后按空格转换",
    "刚输入中文标点后按空格，转换为对应英文标点",
  ],
  ["smart_punctuation_direct_digit", "数字后直出", "数字后输入逗号、句点或冒号时保留 ASCII 标点"],
  ["smart_punctuation_direct_letter", "字母后直出", "字母后输入逗号、句点或冒号时保留 ASCII 标点"],
];

export const pairedPunctuation: PunctuationSwitchCopy = [
  "paired_punctuation",
  "成对标点自动补全",
  "输入左侧符号时自动补全右侧符号，并将光标置于中间",
];

/** 只在报告大写锁定状态的宿主上显示（`HostCapabilities.caps_lock_punctuation`）。 */
export const capsLockPunctuation: PunctuationSwitchCopy = [
  "caps_lock_ascii_punctuation",
  "大写锁定时使用英文标点",
  "大写锁定打开时，中文输入下的标点使用英文标点；正在组字或固定中文标点时不变",
];

/** 「顿号自定义」两个开关，至少留一个（是唯一开着的那个时开关变灰，见 `PunctuationSection`）。 */
export const enumerationCommaSwitches: readonly PunctuationSwitchCopy[] = [
  [
    "enumeration_comma_backslash",
    "顿号自定义 \\",
    "中文标点下 \\ 键输出顿号「、」；至少保留一个顿号键",
  ],
  [
    "enumeration_comma_slash",
    "顿号自定义 /",
    "中文标点下 / 键输出顿号「、」；没有组字时 / 仍打开命令模式，其余位置出顿号",
  ],
];

/** 桌面「标点」选择的选项，把「中文标点」开关和「固定标点」合成一行；取值的含义见 `PunctuationMode`。 */
export const punctuationModeOptions: readonly { value: PunctuationMode; label: string }[] = [
  { value: "chinese", label: "中文标点" },
  { value: "english", label: "英文标点" },
  { value: "always_chinese", label: "始终中文标点（英文模式也用）" },
  { value: "always_english", label: "始终英文标点" },
];

export const punctuationLockOptions: readonly {
  value: NonNullable<PunctuationPreferences["punctuation_lock"]>;
  label: string;
}[] = [
  { value: "follow", label: "跟随中英文状态" },
  { value: "chinese", label: "始终使用中文标点" },
  { value: "english", label: "始终使用英文标点" },
];

export const quoteStyleOptions: readonly { value: QuoteStyle; label: string }[] = [
  { value: "curly", label: "弯引号 “ ” ‘ ’" },
  { value: "corner", label: "直角引号 「 」『 』" },
  { value: "corner_when_traditional", label: "繁体输出时用直角引号" },
];

export const bracketStyleOptions: readonly { value: BracketStyle; label: string }[] = [
  { value: "solid", label: "实心 【 】" },
  { value: "hollow", label: "空心 〖 〗" },
];
