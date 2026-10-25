/** 触屏设置宿主共用的四个底部标签页：首页、社区、统计、我的。「社区」的根页在顶部分「皮肤 / 词库」两类，是完整的社区图库；皮肤页回到「首页」列表里，从「首页」推入。桌面和 2in1 没有底部标签，侧栏照旧。AI 对话从「AI 辅助」进入二级页，避免系统键盘弹出时主导航消失。 */
export type MobilePrimaryPageId = "home" | "community" | "typing-statistics" | "account";

export const mobilePrimaryPageIds: readonly MobilePrimaryPageId[] = [
  "home",
  "community",
  "typing-statistics",
  "account",
];

/** 触屏宿主从「我的」而非「首页」标签页列表打开的页面（有「我的」标签页时 `settingsPageProjections` 不把它们列入该列表），因此它们留在「我的」的导航栈上：该标签保持高亮，切到别的标签再回来仍回到这些页面。 */
const accountTabPages: readonly string[] = [
  "about",
  "feedback",
  "help",
  "usage-reporting",
  "privacy",
];

/** 让嵌套的设置页留在打开它们的标签页的导航栈上：标签页的根页（例如社区）属于自己的标签，「我的」自己的子页面留在「我的」，其他页面（包括皮肤页）都留在「设置」。 */
export function mobileTabForPage(page: string): MobilePrimaryPageId {
  if (mobilePrimaryPageIds.includes(page as MobilePrimaryPageId)) {
    return page as MobilePrimaryPageId;
  }
  return accountTabPages.includes(page) ? "account" : "home";
}

/** Resolves a host route to one of `pages`, or to the shell's fallback page for an id that is not among them. */
export function requestedPage<PageId extends string>(
  value: string | undefined,
  pages: readonly { id: PageId }[],
  fallback: PageId,
): PageId {
  return pages.some((page) => page.id === value) ? (value as PageId) : fallback;
}
