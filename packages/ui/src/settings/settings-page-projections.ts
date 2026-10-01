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
  /** The 插件 page: a host with a pack store, or one that plays or routes something it switches. */
  hasPlugins: boolean;
  mobileHiddenPageIds: readonly SettingsPageId[];
  mobilePageTitle: (id: SettingsPageId, title: string) => string;
}

export interface SettingsPageItem {
  readonly id: SettingsPageId;
  readonly title: string;
  readonly icon: string;
}

/** 导航里的一组页面。`title` 为空的组不显示组名，例如侧栏最前面单独的「首页」。 */
export interface SettingsPageGroup {
  readonly title?: string;
  readonly pages: SettingsPageItem[];
}

export interface SettingsPageProjections {
  availablePages: SettingsPageItem[];
  sidebarGroups: SettingsPageGroup[];
  mobilePrimaryPages: SettingsPageItem[];
  mobileSecondaryGroups: SettingsPageGroup[];
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
  hasPlugins,
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
        (item.id !== "developer" || showDeveloperPage) &&
        (item.id !== "plugins" || hasPlugins) &&
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
    .map(({ title, ids }) => ({
      title,
      pages: ids.flatMap((id) => {
        if (mobilePlatform && !mobileListedPage(id)) return [];
        const item = byId.get(id);
        return item ? [item] : [];
      }),
    }))
    .filter((group) => group.pages.length > 0);
  const home = byId.get("home");
  const sidebarGroups = home ? [{ pages: [home] }, ...groups] : groups;

  const mobilePrimaryPages = mobilePrimaryPageIds.flatMap((id) => {
    const item = availablePages.find((page) => page.id === id);
    return item ? [item] : [];
  });
  const mobileSecondaryGroups = settingsNavGroups
    .map(({ title, ids }) => ({
      title,
      pages: ids.flatMap((id) => {
        if (!mobileListedPage(id)) return [];
        const item = availablePages.find((page) => page.id === id);
        return item ? [item] : [];
      }),
    }))
    .filter((group) => group.pages.length > 0);

  return { availablePages, sidebarGroups, mobilePrimaryPages, mobileSecondaryGroups };
}
