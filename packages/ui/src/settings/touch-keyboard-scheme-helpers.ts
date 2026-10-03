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
  | "tibetan";
export type TouchKeyboardSchemePreferences = {
  enabled: TouchKeyboardScheme[];
  selected?: TouchKeyboardScheme;
};

/** 触屏键盘背后的输入方案；手写不属于任何方案（由平台的手写识别器识别），返回 null。与 client-core 的 `Edition::offers_touch_scheme` 一致。 */
export function touchKeyboardSchemeInputScheme(scheme: TouchKeyboardScheme): InputScheme | null {
  switch (scheme) {
    case "handwriting":
      return null;
    case "quanpin":
    case "nine_key":
      return "quanpin";
    case "xiaohe":
    case "ziranma":
    case "microsoft":
    case "shoudao":
      return "shuangpin";
    case "japanese":
    case "japanese_nine_key":
      return "japanese";
    case "wubi":
    case "korean":
    case "cantonese":
    case "zhuyin":
    case "vietnamese":
    case "tibetan":
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
    }[selected];
  }
  // 韩语、粤拼、注音、越南语和藏文各只有一个键盘，不管文档里记的是哪种布局。
  if (preferences.scheme === "korean") return "韩语 26 键";
  if (preferences.scheme === "cantonese") return "粤拼 26 键";
  if (preferences.scheme === "zhuyin") return "大千注音";
  if (preferences.scheme === "vietnamese") return "越南语 26 键";
  if (preferences.scheme === "tibetan") return "藏文 26 键";
  if (preferences.touch_keyboard_layout === "handwriting") return "手写";
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
];
/** Every touch scheme in picker order; schemes are appended, never reordered. Mirrors `TouchKeyboardScheme::ALL` in client-core. */
export const allTouchKeyboardSchemes = touchKeyboardSchemeOptions.map(([scheme]) => scheme);
/** 没有存过列表的文档显示的方案。粤拼、注音、越南语和藏文需要用户自己打开，这样新增它们不会改变已有的键盘。对应 client-core 的 `TouchKeyboardScheme::DEFAULT_ENABLED`。 */
export const defaultTouchKeyboardSchemes: TouchKeyboardScheme[] = allTouchKeyboardSchemes.filter(
  (scheme) =>
    scheme !== "cantonese" &&
    scheme !== "zhuyin" &&
    scheme !== "vietnamese" &&
    scheme !== "tibetan",
);

export function inferredTouchKeyboardScheme(preferences: Preferences): TouchKeyboardScheme {
  const enabled = preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes;
  const selected = preferences.touch_keyboard_schemes?.selected;
  if (selected && enabled.includes(selected)) return selected;
  const scheme = preferences.scheme;
  // 粤拼、注音、越南语和藏文有各自的触屏键盘；键盘没打开时显示记住的中文方案的键盘。
  if (
    (scheme === "cantonese" ||
      scheme === "zhuyin" ||
      scheme === "vietnamese" ||
      scheme === "tibetan") &&
    enabled.includes(scheme)
  )
    return scheme;
  let inferred = touchSchemeOf(preferences, scheme);
  if (preferences.touch_keyboard_layout === "handwriting" && scheme === "quanpin")
    inferred = "handwriting";
  else if (preferences.touch_keyboard_layout === "nine_key" && scheme !== "korean")
    inferred = scheme === "japanese" ? "japanese_nine_key" : "nine_key";
  return enabled.includes(inferred) ? inferred : (enabled[0] ?? "quanpin");
}

/** 文档方案对应的触屏方案。对没打开自己触屏键盘的文档，粤拼、注音、越南语和藏文对应记住的中文方案的触屏方案，那个也没有时用全拼。 */
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
    case "tibetan": {
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

export function selectTouchKeyboardScheme(
  preferences: Preferences,
  selected: TouchKeyboardScheme,
): Preferences {
  const touch_keyboard_schemes = {
    enabled: preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes,
    selected,
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
  if (selected === "cantonese" || selected === "zhuyin")
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
  const next = selectTouchKeyboardScheme(preferences, selected);
  return { ...next, touch_keyboard_schemes: { enabled: ordered, selected } };
}

/** Select a scheme from the touch keyboard home page, enabling it if necessary. */
export function selectHomeTouchKeyboardScheme(
  preferences: Preferences,
  scheme: TouchKeyboardScheme,
): Preferences {
  const visible = new Set(
    preferences.touch_keyboard_schemes?.enabled ?? defaultTouchKeyboardSchemes,
  );
  visible.add(scheme);
  const enabled = allTouchKeyboardSchemes.filter((value) => visible.has(value));
  const next = selectTouchKeyboardScheme(preferences, scheme);
  return { ...next, touch_keyboard_schemes: { enabled, selected: scheme } };
}
