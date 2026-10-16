import type { FluentIconName } from "../core/fluent-icons";

/**
 * The 设置 tab draws a gear, not the app. Its page icon is the settings glyph
 * used by the shared settings shell.
 */
import { settingsIcon } from "./app-resources";

const settingsTabIcon = settingsIcon;

export function mobileTabIcon(id: string, icon: string): string {
  if (id === "home") return settingsTabIcon;
  return icon;
}

/** HarmonyOS 手机为每个底部标签页绘制的 Fluent 图标。 */
export function harmonyTabIcon(id: string): FluentIconName | undefined {
  if (id === "home") return "settings";
  // 地球与设置、统计、我的三个字形一样宽；people_community 三人并排明显更宽，仍留给「我的」里「我发布的皮肤」那一行。
  if (id === "community") return "globe";
  if (id === "typing-statistics") return "data_bar_vertical";
  if (id === "account") return "person";
  return undefined;
}

const mobilePrimaryTitles: Record<string, string> = {
  home: "设置",
  community: "社区",
  "typing-statistics": "统计",
  account: "我的",
};

/** What a mobile tab is called, which is not always what its page is called. */
export function mobileTabTitle(id: string, title: string): string {
  if (mobilePrimaryTitles[id]) return mobilePrimaryTitles[id];
  return title;
}

/** 桌面标题不适合移动端界面时，触摸宿主对页面的称呼。「我的」中打开「反馈」的那一行保留自己的「帮助与反馈」标签；这里是页面的标题。 */
export function mobilePageTitle(id: string, title: string): string {
  if (mobilePrimaryTitles[id]) return mobilePrimaryTitles[id];
  if (id === "skin") return "皮肤";
  if (id === "expression") return "表达";
  if (id === "appearance") return "候选栏";
  if (id === "screen-keyboard") return "键盘";
  if (id === "developer") return "开发者选项";
  if (id === "feedback") return "反馈";
  return title;
}

/** What a touch host calls a navigation group: the phone has no other way to type, so 更多输入方式 names what the group holds. */
export function mobileGroupTitle(title: string): string {
  if (title === "更多输入方式") return "键盘、语音与手写";
  return title;
}
