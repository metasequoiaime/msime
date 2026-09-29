import { mobilePrimaryPageIds, type MobilePrimaryPageId } from "./settings-navigation-helpers";
import { pages, settingsNavGroups, type SettingsPageId } from "./settings-page-registry";

export interface SettingsPageProjectionOptions {
  mobilePlatform: boolean;
  hasHomePage: boolean;
  hasTypingStatistics: boolean;
  hasVocabularyReview: boolean;
  hasAccount: boolean;
  hasChat: boolean;
  hasCommunity: boolean;
  showFloatingToolbar: boolean;
  showDeveloperPage: boolean;
  mobileHiddenPageIds: readonly SettingsPageId[];
  mobilePageTitle: (id: SettingsPageId, title: string) => string;
}

export interface SettingsPageItem {
  readonly id: SettingsPageId;
  readonly title: string;
  readonly icon: string;
}

export interface SettingsPageProjections {
  availablePages: SettingsPageItem[];
  sidebarGroups: SettingsPageItem[][];
  mobilePrimaryPages: SettingsPageItem[];
  mobileSecondaryGroups: SettingsPageItem[][];
}

/** Projects the current registry into the host-aware page lists used by the settings shell. */
export function settingsPageProjections({
  mobilePlatform,
  hasHomePage,
  hasTypingStatistics,
  hasVocabularyReview,
  hasAccount,
  hasChat,
  hasCommunity,
  showFloatingToolbar,
  showDeveloperPage,
  mobileHiddenPageIds,
  mobilePageTitle,
}: SettingsPageProjectionOptions): SettingsPageProjections {
  const availablePages = pages
    .filter(
      (item) =>
        (item.id !== "home" || hasHomePage) &&
        (item.id !== "typing-statistics" || hasTypingStatistics) &&
        (item.id !== "vocabulary" || hasVocabularyReview) &&
        (item.id !== "account" || hasAccount) &&
        (item.id !== "chat" || hasChat) &&
        (item.id !== "community" || hasCommunity) &&
        (item.id !== "floating-toolbar" || showFloatingToolbar) &&
        (item.id !== "download" || !mobilePlatform) &&
        (item.id !== "developer" || showDeveloperPage) &&
        (item.id !== "more" || mobilePlatform),
    )
    .map((item) => ({
      ...item,
      title: mobilePlatform ? mobilePageTitle(item.id, item.title) : item.title,
    }));

  const mobileListedPage = (id: SettingsPageId): boolean =>
    ((id !== "about" && id !== "feedback") || !hasAccount) &&
    !mobilePrimaryPageIds.includes(id as MobilePrimaryPageId) &&
    !mobileHiddenPageIds.includes(id);

  const sidebarPages = availablePages.filter(
    (item) => item.id !== "more" && !(mobilePlatform && mobileHiddenPageIds.includes(item.id)),
  );
  const byId = new Map(sidebarPages.map((item) => [item.id, item]));
  const groups = settingsNavGroups
    .map((ids) =>
      ids.flatMap((id) => {
        if (mobilePlatform && !mobileListedPage(id)) return [];
        const item = byId.get(id);
        return item ? [item] : [];
      }),
    )
    .filter((group) => group.length > 0);
  const home = byId.get("home");
  const sidebarGroups = home ? [[home], ...groups] : groups;

  const mobilePrimaryPages = mobilePrimaryPageIds.flatMap((id) => {
    const item = availablePages.find((page) => page.id === id);
    return item ? [item] : [];
  });
  const mobileSecondaryGroups = settingsNavGroups
    .map((ids) =>
      ids.flatMap((id) => {
        if (!mobileListedPage(id)) return [];
        const item = availablePages.find((page) => page.id === id);
        return item ? [item] : [];
      }),
    )
    .filter((group) => group.length > 0);

  return { availablePages, sidebarGroups, mobilePrimaryPages, mobileSecondaryGroups };
}
