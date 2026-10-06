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

/** Resolves a host route to one of `pages`, or to the shell's fallback page for an id that is not among them. */
export function requestedPage<PageId extends string>(
  value: string | undefined,
  pages: readonly { id: PageId }[],
  fallback: PageId,
): PageId {
  return pages.some((page) => page.id === value) ? (value as PageId) : fallback;
}
