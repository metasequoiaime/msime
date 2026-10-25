import type { Preferences } from "../core/preferences-model";
import type { FluentIconName } from "../core/fluent-icons";
import { globalThemeTitle } from "../theme/global-theme";
import { touchKeyboardSchemeTitle } from "../settings/touch-keyboard-scheme-helpers";
import { voiceLanguageTitle } from "../voice/voice-language-options";

/** 「首页」根页面可以链接到的页面，以及手机上显示的标题。 */
export interface RootPage {
  id: string;
  title: string;
}

/** 「首页」根页面的一行：它打开的页面、标题、图标，以及（页面有的话）显示在箭头前的当前值。 */
export interface RootSettingsRow {
  id: string;
  title: string;
  icon: FluentIconName;
  value?: string;
}

// 按设计稿的顺序排列设计稿的分组，再加上设计稿没列出的实际页面（「键盘」「候选栏」「剪贴板」和「AI 辅助」），按 Android 的位置安放，这样每个页面都有入口。
const rootGroupLayout: readonly (readonly { id: string; icon: FluentIconName }[])[] = [
  [
    { id: "skin", icon: "color" },
    { id: "screen-keyboard", icon: "keyboard_123" },
    { id: "appearance", icon: "keyboard_dock" },
  ],
  [
    { id: "input", icon: "keyboard" },
    { id: "expression", icon: "local_language" },
    { id: "dictionary", icon: "book" },
  ],
  [
    { id: "voice", icon: "mic" },
    { id: "handwriting", icon: "pen" },
  ],
  [
    { id: "tools", icon: "clipboard" },
    { id: "ai", icon: "sparkle" },
  ],
  [{ id: "developer", icon: "wrench" }],
];

const laidOutIds = new Set(rootGroupLayout.flat().map((entry) => entry.id));

// 宿主可能列出、但设计稿没有对应行的页面所用的图标；这些页面落在最后一组。
const extraPageIcons: Readonly<Record<string, FluentIconName>> = {
  "floating-toolbar": "window",
  plugins: "grid",
  "typing-statistics": "data_bar_vertical",
};

/** 根页面某行显示的当前值，从已保存的偏好读取；页面没有值得显示的单一值时为 undefined。 */
export function rootPageValue(id: string, preferences: Preferences): string | undefined {
  switch (id) {
    case "skin":
      return globalThemeTitle(preferences.global_theme);
    case "input":
      return touchKeyboardSchemeTitle(preferences);
    case "expression":
      return preferences.chinese_punctuation ? "中文标点" : "英文标点";
    case "dictionary":
      return preferences.learning ? "记忆新词已开" : "记忆新词已关";
    case "voice":
      return voiceLanguageTitle(preferences.voice_input?.language);
    default:
      return undefined;
  }
}

/**
 * 「首页」根页面的分组：设计稿的布局按宿主实际提供的页面过滤，保持原有顺序，布局未列出的已提供页面全部归入最后一组，这样宿主新增的页面永远不会从根页面失去入口。
 */
export function rootSettingsGroups(
  rootPages: readonly RootPage[],
  preferences: Preferences,
): RootSettingsRow[][] {
  const titles = new Map(rootPages.map((page) => [page.id, page.title]));
  const row = (id: string, icon: FluentIconName): RootSettingsRow => ({
    id,
    title: titles.get(id) ?? id,
    icon,
    value: rootPageValue(id, preferences),
  });
  const groups = rootGroupLayout.map((group) =>
    group.filter((entry) => titles.has(entry.id)).map((entry) => row(entry.id, entry.icon)),
  );
  const rest = rootPages
    .filter((page) => !laidOutIds.has(page.id))
    .map((page) => row(page.id, extraPageIcons[page.id] ?? "settings"));
  return [...groups, rest].filter((group) => group.length > 0);
}
