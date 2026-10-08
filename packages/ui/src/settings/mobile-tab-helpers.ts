import type { FluentIconName } from "../core/fluent-icons";

/**
 * The 设置 tab draws a gear, not the app. Its page icon is the settings glyph
 * used by the shared settings shell.
 */
const settingsTabIcon = new URL("../assets/settings.svg", import.meta.url).href;

export function mobileTabIcon(id: string, icon: string): string {
  return id === "home" ? settingsTabIcon : icon;
}

/** HarmonyOS 手机设计稿为每个底部标签页绘制的 Fluent 图标；id 不是这四个标签页之一时为 undefined。 */
export function harmonyTabIcon(id: string): FluentIconName | undefined {
  if (id === "home") return "settings";
  if (id === "community") return "people_community";
  if (id === "typing-statistics") return "data_bar_vertical";
  if (id === "account") return "person";
  return undefined;
}

/** What a mobile tab is called, which is not always what its page is called. */
export function mobileTabTitle(id: string, title: string): string {
  if (id === "home") return "设置";
  if (id === "typing-statistics") return "统计";
  if (id === "account") return "我的";
  return title;
}

/** 桌面标题不适合移动端界面时，触摸宿主对页面的称呼。「我的」中打开「反馈」的那一行保留自己的「帮助与反馈」标签；这里是页面的标题。 */
export function mobilePageTitle(id: string, title: string): string {
  if (id === "home") return "设置";
  if (id === "skin") return "皮肤";
  if (id === "expression") return "表达";
  if (id === "appearance") return "候选栏";
  if (id === "screen-keyboard") return "键盘";
  if (id === "shortcuts") return "外接键盘快捷键";
  if (id === "developer") return "开发者选项";
  if (id === "typing-statistics") return "统计";
  if (id === "account") return "我的";
  if (id === "feedback") return "反馈";
  return title;
}

/** What a touch host calls a navigation group: the phone has no other way to type, so 更多输入方式 names what the group holds. */
export function mobileGroupTitle(title: string): string {
  if (title === "更多输入方式") return "键盘、语音与手写";
  return title;
}
