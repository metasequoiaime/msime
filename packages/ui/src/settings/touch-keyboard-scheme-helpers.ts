import type { InputScheme, Preferences } from "../index";
import { isChineseScheme } from "./input-scheme-options";

export type TouchKeyboardScheme =
  | "quanpin"
  | "nine_key"
  | "xiaohe"
  | "ziranma"
  | "microsoft"
  | "shoudao"
  | "wubi"
  | "japanese_nine_key"
  | "japanese"
  | "handwriting"
  | "korean"
  | "cantonese"
  | "zhuyin"
  | "vietnamese"
  | "tibetan"
  | "stroke"
  | "zhuyin_nine_key"
  | "fourteen_key";
export type TouchKeyboardSchemePreferences = {
  enabled: TouchKeyboardScheme[];
  selected?: TouchKeyboardScheme;
};

/** 触屏键盘背后的输入方案；手写不属于任何方案（由平台的手写识别器识别），返回 null，它只在提供中文方案的版本里有。与 client-core 的 `Edition::offers_touch_scheme` 一致。 */
export function touchKeyboardSchemeInputScheme(scheme: TouchKeyboardScheme): InputScheme | null {
  switch (scheme) {
    case "handwriting":
      return null;
    case "quanpin":
    case "nine_key":
    case "fourteen_key":
      return "quanpin";
    case "xiaohe":
    case "ziranma":
    case "microsoft":
    case "shoudao":
      return "shuangpin";
    case "japanese":
    case "japanese_nine_key":
      return "japanese";
    case "zhuyin_nine_key":
      return "zhuyin";
    case "wubi":
    case "korean":
    case "cantonese":
    case "zhuyin":
    case "vietnamese":
    case "tibetan":
    case "stroke":
      return scheme;
  }
}

/** 五笔触屏方案的标题：只有一个五笔键盘，标题跟随 `wubi_profile`。 */
export function wubiProfileTitle(profile: Preferences["wubi_profile"]): string {
  return profile === "wubi98" ? "98 五笔" : "86 五笔";
}

export function touchKeyboardSchemeTitle(preferences: Preferences): string {
  const selected = preferences.touch_keyboard_schemes?.selected;
  if (selected === "wubi") return wubiProfileTitle(preferences.wubi_profile);
  if (selected) {
    return {
      quanpin: "全拼 26 键",
      nine_key: "全拼 9 键",
      xiaohe: "小鹤双拼",
      ziranma: "自然码双拼",
      microsoft: "微软双拼",
      shoudao: "首道双拼",
      wubi: "86 五笔",
      japanese_nine_key: "日语 9 键",
      japanese: "日语 26 键",
      handwriting: "手写",
      korean: "韩语 26 键",
      cantonese: "粤拼 26 键",
      zhuyin: "大千注音",
      vietnamese: "越南语 26 键",
      tibetan: "藏文 26 键",
      stroke: "笔画",
      zhuyin_nine_key: "注音 9 键",
      fourteen_key: "全拼 14 键",
    }[selected];
  }
  // 韩语、粤拼、注音、越南语、藏文和笔画各只有一个键盘，不管文档里记的是哪种布局。
  if (preferences.scheme === "korean") return "韩语 26 键";
  if (preferences.scheme === "cantonese") return "粤拼 26 键";
  if (preferences.scheme === "zhuyin") return "大千注音";
  if (preferences.scheme === "vietnamese") return "越南语 26 键";
  if (preferences.scheme === "tibetan") return "藏文 26 键";
  if (preferences.scheme === "stroke") return "笔画";
  if (preferences.touch_keyboard_layout === "handwriting") return "手写";
  // 14 键只配全拼；别的方案记着这个布局时按各自的键盘命名。
  if (preferences.touch_keyboard_layout === "fourteen_key" && preferences.scheme === "quanpin")
    return "全拼 14 键";
  if (preferences.touch_keyboard_layout === "nine_key")
    return preferences.scheme === "japanese" ? "日语 9 键" : "全拼 9 键";
  if (preferences.scheme === "japanese") return "日语 26 键";
  if (preferences.scheme === "wubi") return wubiProfileTitle(preferences.wubi_profile);
  if (preferences.scheme === "shuangpin") return `${preferences.shuangpin_profile} 双拼`;
  return "全拼 26 键";
}

export const touchKeyboardSchemeOptions: [TouchKeyboardScheme, string][] = [
  ["quanpin", "全拼 26 键"],
  ["nine_key", "全拼 9 键"],
  ["xiaohe", "小鹤双拼"],
  ["ziranma", "自然码双拼"],
  ["microsoft", "微软双拼"],
  ["shoudao", "首道双拼"],
  ["wubi", "86 五笔"],
  ["japanese_nine_key", "日语 9 键"],
  ["japanese", "日语 26 键"],
  ["handwriting", "手写"],
  ["korean", "韩语 26 键"],
  ["cantonese", "粤拼 26 键"],
  ["zhuyin", "大千注音"],
  ["vietnamese", "越南语 26 键"],
  ["tibetan", "藏文 26 键"],
  ["stroke", "笔画"],
  ["zhuyin_nine_key", "注音 9 键"],
  ["fourteen_key", "全拼 14 键"],
];
/** Every touch scheme in picker order; schemes are appended, never reordered. Mirrors `TouchKeyboardScheme::ALL` in client-core. */
export const allTouchKeyboardSchemes = touchKeyboardSchemeOptions.map(([scheme]) => scheme);
/** 没有存过列表的文档显示的方案：粤拼、注音、越南语、藏文、笔画和全拼 14 键以外的全部，日语和韩语也在里面。对应 client-core 的 `TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED`：没有列表的文档只可能出自默认值改成只有中文之前的版本，按那时的默认列表读，升级的用户键盘不变。新装只启用中文方案（`TouchKeyboardScheme::DEFAULT_ENABLED`），这个列表由 client-core 显式写进文档，不经过这里的回退。 */
export const defaultTouchKeyboardSchemes: TouchKeyboardScheme[] = allTouchKeyboardSchemes.filter(
  (scheme) =>
    scheme !== "cantonese" &&
    scheme !== "zhuyin" &&
    scheme !== "vietnamese" &&
    scheme !== "tibetan" &&
    scheme !== "stroke" &&
    scheme !== "zhuyin_nine_key" &&
    scheme !== "fourteen_key",
);

/**
 * `handwritingScheme` 是手写写进偏好 `scheme` 的方案，即运行中版本的默认方案（`HostCapabilities.edition.default_scheme`），缺省是全拼。手写识别由平台识别器完成，不经过 Engine 的方案；五笔版里手写写 `wubi`，免得 host-api 把全拼当作本版本不含的方案回退。与 Android 的 `KeyboardScheme.engineScheme` 一致。
 */
export function inferredTouchKeyboardScheme(
  preferences: Preferences,
  handwritingScheme: InputScheme = "quanpin",
): TouchKeyboardScheme {
  const enabled = preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes;
  const selected = preferences.touch_keyboard_schemes?.selected;
  if (selected && enabled.includes(selected)) return selected;
  const scheme = preferences.scheme;
  // 注音的九键布局只有 Android 写（zhuyin_nine_key 打开且布局是 nine_key 时）；桌面为全拼九宫格留下的 nine_key 不算。
  if (
    scheme === "zhuyin" &&
    preferences.touch_keyboard_layout === "nine_key" &&
    enabled.includes("zhuyin_nine_key")
  )
    return "zhuyin_nine_key";
  // 粤拼、注音、越南语、藏文和笔画有各自的触屏键盘（笔画键盘在 26 键和九键布局下都显示）；键盘没打开时显示记住的中文方案的键盘。
  if (
    (scheme === "cantonese" ||
      scheme === "zhuyin" ||
      scheme === "vietnamese" ||
      scheme === "tibetan" ||
      scheme === "stroke") &&
    enabled.includes(scheme)
  )
    return scheme;
  let inferred = touchSchemeOf(preferences, scheme);
  if (preferences.touch_keyboard_layout === "handwriting" && scheme === handwritingScheme)
    inferred = "handwriting";
  else if (preferences.touch_keyboard_layout === "nine_key" && scheme !== "korean")
    inferred = scheme === "japanese" ? "japanese_nine_key" : "nine_key";
  else if (preferences.touch_keyboard_layout === "fourteen_key" && scheme === "quanpin")
    inferred = "fourteen_key";
  return enabled.includes(inferred) ? inferred : (enabled[0] ?? "quanpin");
}

/** 文档方案对应的触屏方案。对没打开自己触屏键盘的文档，粤拼、注音、越南语、藏文和笔画对应记住的中文方案的触屏方案，那个也没有时用全拼。 */
function touchSchemeOf(
  preferences: Preferences,
  scheme: Preferences["scheme"],
): TouchKeyboardScheme {
  switch (scheme) {
    case "shuangpin":
      return preferences.shuangpin_profile;
    case "quanpin":
    case "wubi":
    case "japanese":
    case "korean":
      return scheme;
    case "cantonese":
    case "zhuyin":
    case "vietnamese":
    case "tibetan":
    case "stroke": {
      const remembered = preferences.last_chinese_scheme;
      return remembered === "quanpin" || remembered === "shuangpin" || remembered === "wubi"
        ? touchSchemeOf(preferences, remembered)
        : "quanpin";
    }
  }
}

/** 选日文、韩文、越南语或藏文后要回到的中文方案；在这几个之间切换时保留已经记住的那个。 */
function rememberedChineseScheme(preferences: Preferences): Preferences["last_chinese_scheme"] {
  return isChineseScheme(preferences.scheme) ? preferences.scheme : preferences.last_chinese_scheme;
}

/** 选中一个触屏方案后的偏好；`handwritingScheme` 见 {@link inferredTouchKeyboardScheme}。 */
export function selectTouchKeyboardScheme(
  preferences: Preferences,
  selected: TouchKeyboardScheme,
  handwritingScheme: InputScheme = "quanpin",
): Preferences {
  const touch_keyboard_schemes = {
    enabled: preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes,
    selected,
  };
  if (selected === "handwriting" && handwritingScheme !== "quanpin")
    return {
      ...preferences,
      scheme: handwritingScheme,
      last_chinese_scheme: isChineseScheme(handwritingScheme)
        ? handwritingScheme
        : rememberedChineseScheme(preferences),
      touch_keyboard_layout: "handwriting",
      touch_keyboard_schemes,
    };
  if (["xiaohe", "ziranma", "microsoft", "shoudao"].includes(selected))
    return {
      ...preferences,
      scheme: "shuangpin",
      last_chinese_scheme: "shuangpin",
      shuangpin_profile: selected as Preferences["shuangpin_profile"],
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes,
    };
  if (selected === "japanese" || selected === "japanese_nine_key")
    return {
      ...preferences,
      scheme: "japanese",
      last_chinese_scheme: rememberedChineseScheme(preferences),
      touch_keyboard_layout: selected === "japanese_nine_key" ? "nine_key" : "twenty_six_key",
      touch_keyboard_schemes,
    };
  if (selected === "korean")
    return {
      ...preferences,
      scheme: "korean",
      last_chinese_scheme: rememberedChineseScheme(preferences),
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes,
    };
  if (selected === "vietnamese" || selected === "tibetan")
    return {
      ...preferences,
      scheme: selected,
      last_chinese_scheme: rememberedChineseScheme(preferences),
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes,
    };
  if (selected === "zhuyin_nine_key")
    return {
      ...preferences,
      scheme: "zhuyin",
      last_chinese_scheme: "zhuyin",
      touch_keyboard_layout: "nine_key",
      touch_keyboard_schemes,
    };
  if (selected === "cantonese" || selected === "zhuyin" || selected === "stroke")
    return {
      ...preferences,
      scheme: selected,
      last_chinese_scheme: selected,
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes,
    };
  if (selected === "wubi")
    return {
      ...preferences,
      scheme: "wubi",
      last_chinese_scheme: "wubi",
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes,
    };
  return {
    ...preferences,
    scheme: "quanpin",
    last_chinese_scheme: "quanpin",
    touch_keyboard_layout:
      selected === "nine_key"
        ? "nine_key"
        : selected === "fourteen_key"
          ? "fourteen_key"
          : selected === "handwriting"
            ? "handwriting"
            : "twenty_six_key",
    touch_keyboard_schemes,
  };
}

/** Toggle a touch scheme while keeping one visible and a valid selected scheme. */
export function updateTouchKeyboardSchemeEnabled(
  preferences: Preferences,
  scheme: TouchKeyboardScheme,
  enabled: boolean,
  selectedTouchKeyboardScheme = inferredTouchKeyboardScheme(preferences),
  handwritingScheme: InputScheme = "quanpin",
): Preferences | null {
  const visible = new Set(
    preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes,
  );
  if (enabled) visible.add(scheme);
  else visible.delete(scheme);
  if (visible.size === 0) return null;
  const ordered = allTouchKeyboardSchemes.filter((value) => visible.has(value));
  const selected = visible.has(selectedTouchKeyboardScheme)
    ? selectedTouchKeyboardScheme
    : ordered[0];
  const next = selectTouchKeyboardScheme(preferences, selected, handwritingScheme);
  return { ...next, touch_keyboard_schemes: { enabled: ordered, selected } };
}

/** Select a scheme from the touch keyboard home page, enabling it if necessary. */
export function selectHomeTouchKeyboardScheme(
  preferences: Preferences,
  scheme: TouchKeyboardScheme,
  handwritingScheme: InputScheme = "quanpin",
): Preferences {
  const visible = new Set(
    preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes,
  );
  visible.add(scheme);
  const enabled = allTouchKeyboardSchemes.filter((value) => visible.has(value));
  const next = selectTouchKeyboardScheme(preferences, scheme, handwritingScheme);
  return { ...next, touch_keyboard_schemes: { enabled, selected: scheme } };
}

// ---- 语言与方案（HarmonyOS 手机） ----

export type TouchKeyboardLanguageId =
  | "mandarin"
  | "cantonese"
  | "japanese"
  | "korean"
  | "vietnamese"
  | "tibetan";

/** 「语言与方案」卡片中的一种语言：徽标、名称，以及按面板顺序排列的触摸方案。 */
export interface TouchKeyboardLanguage {
  id: TouchKeyboardLanguageId;
  badge: string;
  title: string;
  schemes: readonly TouchKeyboardScheme[];
}

/** 按语言分组的触摸方案，移植自 Android `TypingPage` 的 `Language` 枚举。英文没有自己的触摸方案（它是每种键盘的一种模式），所以这里不算一种语言。 */
export const touchKeyboardLanguages: readonly TouchKeyboardLanguage[] = [
  {
    id: "mandarin",
    badge: "汉",
    title: "普通话",
    schemes: [
      "quanpin",
      "fourteen_key",
      "nine_key",
      "xiaohe",
      "ziranma",
      "microsoft",
      "shoudao",
      "wubi",
      "zhuyin",
      "zhuyin_nine_key",
      "stroke",
      "handwriting",
    ],
  },
  { id: "cantonese", badge: "粤", title: "粤语", schemes: ["cantonese"] },
  { id: "japanese", badge: "あ", title: "日语", schemes: ["japanese", "japanese_nine_key"] },
  { id: "korean", badge: "한", title: "韩语", schemes: ["korean"] },
  { id: "vietnamese", badge: "Vi", title: "越南语", schemes: ["vietnamese"] },
  { id: "tibetan", badge: "བོད", title: "藏语", schemes: ["tibetan"] },
];

/** 本宿主和本版本提供的该语言方案（`available` 是设置页显示的已过滤方案列表），按该语言自身的顺序排列。 */
export function offeredLanguageSchemes(
  language: TouchKeyboardLanguage,
  available: readonly TouchKeyboardScheme[],
): TouchKeyboardScheme[] {
  return language.schemes.filter((scheme) => available.includes(scheme));
}

/** 语言行显示的方案：选中的方案属于该语言时就是它，否则是该语言按已启用列表顺序的第一个已启用方案；该语言提供的方案都未启用时为 null。 */
export function shownLanguageScheme(
  language: TouchKeyboardLanguage,
  selected: TouchKeyboardScheme,
  enabled: readonly TouchKeyboardScheme[],
  available: readonly TouchKeyboardScheme[],
): TouchKeyboardScheme | null {
  const offered = offeredLanguageSchemes(language, available);
  if (offered.includes(selected) && enabled.includes(selected)) return selected;
  return enabled.find((scheme) => offered.includes(scheme)) ?? null;
}

/** 移除某种语言时键盘退回的方案：按选择器顺序，另一种语言中第一个已启用且已提供的方案；移除后会没有任何方案启用时为 null。普通话永远不会被移除。 */
export function languageRemovalFallback(
  language: TouchKeyboardLanguage,
  enabled: readonly TouchKeyboardScheme[],
  available: readonly TouchKeyboardScheme[],
): TouchKeyboardScheme | null {
  if (language.id === "mandarin") return null;
  return (
    allTouchKeyboardSchemes.find(
      (scheme) =>
        !language.schemes.includes(scheme) &&
        enabled.includes(scheme) &&
        available.includes(scheme),
    ) ?? null
  );
}
