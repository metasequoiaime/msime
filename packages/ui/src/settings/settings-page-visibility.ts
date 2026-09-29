import type { SettingsPageId } from "./mobile-navigation";

const formExcludedPages: readonly SettingsPageId[] = [
  "typing-statistics",
  "vocabulary",
  "account",
  "chat",
  "more",
  "community",
];

const reloadExcludedPages: readonly SettingsPageId[] = [
  "typing-statistics",
  "vocabulary",
  "account",
  "chat",
  "community",
];

/** Whether the shared settings form is mounted for a page. */
export function isSettingsFormPage(page: SettingsPageId) {
  return !formExcludedPages.includes(page);
}

/** Whether the page can discard its draft through the reload action. */
export function canReloadSettingsPage(page: SettingsPageId) {
  return !reloadExcludedPages.includes(page);
}
