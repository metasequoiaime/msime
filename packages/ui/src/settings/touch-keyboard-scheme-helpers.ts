import type { Preferences } from "../index";

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
  | "thoughtful_reply";
export type TouchKeyboardSchemePreferences = {
  enabled: TouchKeyboardScheme[];
  selected?: TouchKeyboardScheme;
};

export function touchKeyboardSchemeTitle(preferences: Preferences): string {
  const selected = preferences.touch_keyboard_schemes?.selected;
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
      thoughtful_reply: "高情商回复",
    }[selected];
  }
  if (preferences.touch_keyboard_layout === "handwriting") return "手写";
  if (preferences.touch_keyboard_layout === "nine_key")
    return preferences.scheme === "japanese" ? "日语 9 键" : "全拼 9 键";
  if (preferences.scheme === "japanese") return "日语 26 键";
  if (preferences.scheme === "wubi") return "86 五笔";
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
  ["thoughtful_reply", "高情商回复"],
];
export const allTouchKeyboardSchemes = touchKeyboardSchemeOptions.map(([scheme]) => scheme);

export function inferredTouchKeyboardScheme(preferences: Preferences): TouchKeyboardScheme {
  const enabled = preferences.touch_keyboard_schemes?.enabled ?? allTouchKeyboardSchemes;
  const selected = preferences.touch_keyboard_schemes?.selected;
  if (selected && enabled.includes(selected)) return selected;
  let inferred: TouchKeyboardScheme =
    preferences.scheme === "shuangpin" ? preferences.shuangpin_profile : preferences.scheme;
  if (preferences.touch_keyboard_layout === "handwriting" && preferences.scheme === "quanpin")
    inferred = "handwriting";
  else if (preferences.touch_keyboard_layout === "nine_key")
    inferred = preferences.scheme === "japanese" ? "japanese_nine_key" : "nine_key";
  return enabled.includes(inferred) ? inferred : (enabled[0] ?? "quanpin");
}

export function selectTouchKeyboardScheme(
  preferences: Preferences,
  selected: TouchKeyboardScheme,
): Preferences {
  const touch_keyboard_schemes = {
    enabled: preferences.touch_keyboard_schemes?.enabled ?? allTouchKeyboardSchemes,
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
      last_chinese_scheme:
        preferences.scheme === "japanese" ? preferences.last_chinese_scheme : preferences.scheme,
      touch_keyboard_layout: selected === "japanese_nine_key" ? "nine_key" : "twenty_six_key",
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
  const visible = new Set(preferences.touch_keyboard_schemes?.enabled ?? allTouchKeyboardSchemes);
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
  const visible = new Set(preferences.touch_keyboard_schemes?.enabled ?? allTouchKeyboardSchemes);
  visible.add(scheme);
  const enabled = allTouchKeyboardSchemes.filter((value) => visible.has(value));
  const next = selectTouchKeyboardScheme(preferences, scheme);
  return { ...next, touch_keyboard_schemes: { enabled, selected: scheme } };
}
