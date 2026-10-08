import type { Preferences } from "../index";
import { FluentIcon, type FluentIconName } from "../core/fluent-icons";
import { NavGroup, NavRow } from "../core/platform-controls";
import { themeEntry } from "../theme/global-theme";
import { touchKeyboardSchemeTitle } from "../settings/touch-keyboard-scheme-helpers";

/** 「设置」根页面可以链接到的页面，以及手机上显示的标题。 */
export interface RootPage {
  id: string;
  title: string;
}

/** 「设置」根页面的一行：它打开的页面、标题、图标，以及（页面有的话）显示在箭头前的当前值。 */
export interface RootSettingsRow {
  id: string;
  title: string;
  icon: FluentIconName;
  value?: string;
}

// 按设计稿的顺序排列设计稿的分组，再加上设计稿没列出的实际页面（「键盘」「候选栏」「外接键盘快捷键」「剪贴板」和「AI 辅助」），按 Android 的位置安放，这样每个页面都有入口。
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
    { id: "shortcuts", icon: "keyboard_shift" },
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

/** 「皮肤」行和桌面首页卡片显示的皮肤名称：目录里的标题，用户自己设计的皮肤则为「我的皮肤」。 */
export function skinTitle(preferences: Preferences): string {
  const selected = themeEntry(preferences.global_theme).id;
  return selected === "custom" ? "我的皮肤" : themeEntry(selected).title;
}

/** 「语音输入」行显示的语音识别语言名称。系统识别器的 locale id 和各服务商的简写都有映射；空值是默认的「普通话」；没有映射的值直接显示代码本身，不去猜测。 */
export function voiceLanguageTitle(language: string | undefined): string {
  const code = (language ?? "").trim();
  switch (code.toLowerCase()) {
    case "":
    case "zh":
    case "zh-cn":
      return "普通话";
    case "yue":
    case "zh-hk":
      return "粤语";
    case "en":
    case "en-us":
      return "英语";
    case "ja":
    case "ja-jp":
      return "日语";
    case "auto":
      return "普通话 + 英语";
    default:
      return code;
  }
}

/** 根页面某行显示的当前值，从已保存的偏好读取；页面没有值得显示的单一值时为 undefined。 */
export function rootPageValue(id: string, preferences: Preferences): string | undefined {
  switch (id) {
    case "skin":
      return skinTitle(preferences);
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
 * 「设置」根页面的分组：设计稿的布局按宿主实际提供的页面过滤，保持原有顺序，布局未列出的已提供页面全部归入最后一组，这样宿主新增的页面永远不会从根页面失去入口。
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

/** 根页面的一行：带 Fluent 图标的 `NavRow`。 */
export function RootSettingsRowView({
  row,
  onOpenPage,
}: {
  row: RootSettingsRow;
  onOpenPage: (page: string) => void;
}) {
  return (
    <NavRow
      icon={<FluentIcon name={row.icon} size={20} />}
      title={row.title}
      value={row.value}
      onClick={() => onOpenPage(row.id)}
    />
  );
}

/** 「设置」根页面的分组行：每组一张无边框卡片，间距 20px，不显示组标题。 */
export function RootSettingsList({
  groups,
  onOpenPage,
}: {
  groups: readonly (readonly RootSettingsRow[])[];
  onOpenPage: (page: string) => void;
}) {
  return (
    <div className="flex flex-col gap-5">
      {groups.map((group) => (
        <NavGroup key={group[0].id}>
          {group.map((row) => (
            <RootSettingsRowView key={row.id} row={row} onOpenPage={onOpenPage} />
          ))}
        </NavGroup>
      ))}
    </div>
  );
}
