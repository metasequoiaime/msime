import type { SettingsPageId } from "./settings-page-registry";

export interface SettingsPageTitleItem {
  id: SettingsPageId;
  title: string;
}

export interface SettingsPageLinkItem extends SettingsPageTitleItem {
  icon: string;
}

/** Resolves the visible page heading and keeps the appearance fallback for unknown routes. */
export function settingsPageTitle(pages: readonly SettingsPageTitleItem[], page: SettingsPageId) {
  return pages.find((item) => item.id === page)?.title ?? "外观";
}

/** Projects page descriptors into the compact links used by the More settings list. */
export function settingsPageLinks(pages: readonly SettingsPageLinkItem[]) {
  return pages.map(({ id, title, icon }) => ({ id, title, icon }));
}
