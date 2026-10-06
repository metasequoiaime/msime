/**
 * The 设置 tab draws a gear, not the app. Its page icon is the settings glyph
 * used by the shared settings shell.
 */
const settingsTabIcon = new URL("../assets/settings.svg", import.meta.url).href;

export function mobileTabIcon(id: string, icon: string): string {
  return id === "home" ? settingsTabIcon : icon;
}

/** What a mobile tab is called, which is not always what its page is called. */
export function mobileTabTitle(id: string, title: string): string {
  if (id === "home") return "设置";
  if (id === "typing-statistics") return "统计";
  if (id === "account") return "我的";
  return title;
}

/** What a touch host calls a page when its desktop title does not fit the mobile surface. */
export function mobilePageTitle(id: string, title: string): string {
  if (id === "appearance") return "候选栏";
  if (id === "screen-keyboard") return "键盘";
  if (id === "shortcuts") return "外接键盘快捷键";
  if (id === "account") return "我的";
  return title;
}

/** What a touch host calls a navigation group: the phone has no other way to type, so 更多输入方式 names what the group holds. */
export function mobileGroupTitle(title: string): string {
  if (title === "更多输入方式") return "键盘、语音与手写";
  return title;
}
