import type { ExportedSettingsPageId as SettingsPageId } from "./settings-page-registry";

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

/** 页脚显示「恢复默认设置」的偏好页。关于、反馈、帮助、维护与诊断这类页面不改偏好，在那里出现这个按钮会让人以为它只恢复当前页。 */
const restoreDefaultsPages: readonly SettingsPageId[] = [
  "skin",
  "appearance",
  "floating-toolbar",
  "input",
  "expression",
  "ai",
  "shortcuts",
  "dictionary",
  "screen-keyboard",
  "voice",
  "handwriting",
  "tools",
  "plugins",
];

/** Whether the shared settings form is mounted for a page. */
export function isSettingsFormPage(page: SettingsPageId) {
  return !formExcludedPages.includes(page);
}

/** Whether the page can discard its draft through the reload action. */
export function canReloadSettingsPage(page: SettingsPageId) {
  return !reloadExcludedPages.includes(page);
}

/** 页面的页脚是否显示「恢复默认设置」。 */
export function canRestoreDefaultsOnPage(page: SettingsPageId) {
  return restoreDefaultsPages.includes(page);
}
