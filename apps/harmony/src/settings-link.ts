/**
 * 键盘直接打开的设置：一个页面，可带一个页面里的子目标。
 *
 * 页面是功能面板的「词库」「反馈」「关于」、皮肤选择器的「更多皮肤」，「添加语言」打开的「输入」，以及长按工具栏按钮打开的「屏幕键盘」（工具栏的显示方式和工具开关在这一页）。与 `EntryAbility` 从 Want 中接受的列表（`SettingsPageLink.PAGES`）相同，scripts/test-harmony-settings-links.py 核对两份一致；其他值都不是本页承诺过的链接，而改为打开「输入」（未知页面 id 就会解析到它）比停在原处更让人摸不着头脑。
 */
export const LINKED_PAGES: readonly string[] = [
  "input",
  "dictionary",
  "feedback",
  "about",
  "skin",
  "screen-keyboard",
];

/**
 * 「屏幕键盘」页的子目标：长按九键左侧符号栏时带上的那张表的共享偏好名，设置页打开「按键布局」并直接进入那张表的编辑页（`nineKeySidebarLinkTargets`）。与 `SettingsPageLink.SCREEN_KEYBOARD_TARGETS` 相同，同一个脚本核对。
 */
export const LINKED_SCREEN_KEYBOARD_TARGETS: readonly string[] = [
  "touch_nine_key_sidebar_symbols",
  "touch_nine_key_digit_sidebar_symbols",
];

export interface SettingsLink {
  page: string;
  /** 页面里要直接打开的那一处；没有时只打开页面。 */
  target?: string;
}

export function linkedPage(value: unknown): string | undefined {
  return typeof value === "string" && LINKED_PAGES.includes(value) ? value : undefined;
}

/** 页面接受的子目标；不是字符串、不认得或不属于这个页面时为 undefined，链接照旧只打开页面。 */
export function linkedTarget(page: string, value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  return page === "screen-keyboard" && LINKED_SCREEN_KEYBOARD_TARGETS.includes(value)
    ? value
    : undefined;
}

/** 宿主送来的页面和子目标；页面不可链接时为 undefined。只带页面的旧链接照旧是一个只有页面的链接。 */
export function settingsLink(page: unknown, target?: unknown): SettingsLink | undefined {
  const linked = linkedPage(page);
  if (!linked) return undefined;
  const sub = linkedTarget(linked, target);
  return sub ? { page: linked, target: sub } : { page: linked };
}

/** 宿主 `pendingPage()` 的回复 `{"ok":true,"value":页面|null,"target":子目标|null}`；旧宿主没有 `target`，读出来只有页面。 */
export function pendingSettingsLink(raw: string): SettingsLink | undefined {
  try {
    const reply = JSON.parse(raw) as { ok?: unknown; value?: unknown; target?: unknown };
    return reply.ok === true ? settingsLink(reply.value, reply.target) : undefined;
  } catch {
    return undefined;
  }
}
