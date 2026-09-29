/** The four primary tabs shared by touch settings hosts. */
export type MobilePrimaryPageId = "home" | "community" | "typing-statistics" | "account";

export const mobilePrimaryPageIds: readonly MobilePrimaryPageId[] = [
  "home",
  "community",
  "typing-statistics",
  "account",
];

/** Keeps nested settings pages on the 设置 tab's navigation stack. */
export function mobileTabForPage(page: string): MobilePrimaryPageId {
  return mobilePrimaryPageIds.includes(page as MobilePrimaryPageId)
    ? (page as MobilePrimaryPageId)
    : "home";
}

/** Resolves a host route through aliases, known pages, and the shell's fallback page. */
export function requestedPage<PageId extends string>(
  value: string | undefined,
  pages: readonly { id: PageId }[],
  aliases: Readonly<Record<string, PageId>>,
  fallback: PageId,
): PageId {
  if (value !== undefined && Object.hasOwn(aliases, value)) return aliases[value];
  return pages.some((page) => page.id === value) ? (value as PageId) : fallback;
}
