import { settingsNavGroups, type SettingsPageId } from "./settings-page-registry";

/** 旧版 macOS 设置窗口的侧栏分组。现在直接取 `settingsNavGroups` 的页面 id，不再单独维护一份顺序。 */
export const macosSidebarGroups: readonly (readonly SettingsPageId[])[] = settingsNavGroups.map(
  (group) => group.ids,
);
