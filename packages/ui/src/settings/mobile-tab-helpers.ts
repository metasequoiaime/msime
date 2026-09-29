/**
 * The 键盘 tab draws a keyboard, not the app. Its page icon is the keyboard
 * artwork used by the screen keyboard page.
 */
const keyboardTabIcon = new URL("../assets/screen-keyboard.svg", import.meta.url).href;

export function mobileTabIcon(id: string, icon: string): string {
  return id === "home" ? keyboardTabIcon : icon;
}

/** What a mobile tab is called, which is not always what its page is called. */
export function mobileTabTitle(id: string, title: string): string {
  if (id === "home") return "键盘";
  if (id === "typing-statistics") return "统计";
  if (id === "account") return "我的";
  return title;
}
