export type MixedInputPreferences = {
  /** 候选混输总开关（Windows `utility.mixed_candidates`）。后加的可选成员：Rust 端开着时不写这个键，缺省按开处理；关掉后各分开关原值保留。 */
  enabled?: boolean;
  english: boolean;
  minimum_prefix: number;
  /** 中文状态下首页最多展示几个英文候选（Windows `cn_en_mixed_input_first_page_count`，1–3）。后加的可选成员：为 1 时不写这个键，缺省按 1 处理。 */
  first_page_count?: number;
  emoji: boolean;
  kaomoji: boolean;
  /** 日文混输（实验）。后加的可选成员：Rust 端关着时不写这个键，旧文档和只写前四项的宿主都没有它，缺省按关处理；也因此不放进 `defaultMixedInput`。 */
  japanese?: boolean;
  /** 符号混输（打拼音出符号）。后加的可选成员：Rust 端开着时不写这个键，旧文档和只写前四项的宿主都没有它，缺省按开处理；也因此不放进 `defaultMixedInput`。 */
  symbols?: boolean;
  /** 快捷短语混输（编码恰好是某条快捷短语的编码时，短语排在普通候选之后）。后加的可选成员：只对声明 `quick_phrase_candidates` 能力的宿主（macOS、Linux）生效，它们此前无条件混入，旧文档缺键读作开；新装默认关，开着时不写键。 */
  quick_phrase?: boolean;
  /** 快捷短语参与调频：常越过整组快捷短语选别的候选时整组后退，最多退到首页末位；选中组内短语时整组前移。后加的可选成员，缺键读作开，只在混输打开时起作用。 */
  quick_phrase_frequency?: boolean;
};

export const defaultMixedInput: MixedInputPreferences = {
  english: true,
  minimum_prefix: 5,
  emoji: false,
  kaomoji: false,
};

/** Windows 和 macOS 上 emoji 混输默认开，与 client-core 的 `source_mixed_emoji_default` 一致。 */
const sourceEmojiMixedInput: MixedInputPreferences = { ...defaultMixedInput, emoji: true };

/** 文档里还没有 `mixed_input` 时按宿主平台取的缺省：Windows 和 macOS 的 emoji 混输开（Rust 按编译目标取 `cfg!(any(windows, target_os = "macos"))`），其他平台和没有宿主时关。 */
export function mixedInputDefaultsFor(platform?: string | null): MixedInputPreferences {
  return platform === "macos" || platform === "windows" ? sourceEmojiMixedInput : defaultMixedInput;
}

/** 仅投影控件状态，不补写偏好或改变英文触发字符数。 */
export function mixedInputControlState(preferences: MixedInputPreferences) {
  const enabled = preferences.enabled ?? true;
  return {
    enabled,
    symbolsChecked: preferences.symbols ?? true,
    japaneseChecked: preferences.japanese ?? false,
    englishDisabled: !enabled,
    minimumPrefixDisabled: !enabled || !preferences.english,
    firstPageCountDisabled: !enabled || !preferences.english,
    firstPageCount: preferences.first_page_count ?? 1,
    quickPhraseChecked: preferences.quick_phrase ?? true,
    quickPhraseFrequencyChecked: preferences.quick_phrase_frequency ?? true,
  };
}
