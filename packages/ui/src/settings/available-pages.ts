import { pages } from "./mobile-navigation";

export interface AvailablePageCapabilities {
  home: boolean;
  typingStatistics: boolean;
  vocabularyReview: boolean;
  account: boolean;
  chat: boolean;
  community: boolean;
  floatingToolbar: boolean;
  /** The 插件 page: a host with a pack store, or one that plays or routes something it switches. */
  plugins: boolean;
  mobile: boolean;
}

/** Pages with a host-backed entry point, plus the form-factor-specific navigation pages. */
export function availableSettingsPages(capabilities: AvailablePageCapabilities) {
  return pages.filter(
    (item) =>
      (item.id !== "home" || capabilities.home) &&
      (item.id !== "typing-statistics" || capabilities.typingStatistics) &&
      (item.id !== "vocabulary" || capabilities.vocabularyReview) &&
      (item.id !== "account" || capabilities.account) &&
      (item.id !== "chat" || capabilities.chat) &&
      (item.id !== "community" || capabilities.community) &&
      (item.id !== "floating-toolbar" || capabilities.floatingToolbar) &&
      (item.id !== "plugins" || capabilities.plugins) &&
      (item.id !== "more" || capabilities.mobile),
  );
}
