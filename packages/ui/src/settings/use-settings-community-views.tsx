import type { SettingsPageId } from "./settings-page-registry";
import { skinGridTabs } from "./skin-grid";
import type { SettingsShellView } from "./use-settings-shell-state";
import { useSettingsTabs } from "./use-settings-tabs";

export interface UseSettingsCommunityViewsOptions {
  page: SettingsPageId;
  mobile: boolean;
  harmony: boolean;
  skinView: SettingsShellView;
  pluginView: SettingsShellView;
  hasCommunitySkins: boolean;
  hasCommunityCandidateSkins: boolean;
  hasCommunityPlugins: boolean;
}

/** 管理设置外壳中的皮肤分类标签，以及皮肤和插件社区图库对表单的覆盖状态。 */
export function useSettingsCommunityViews({
  page,
  mobile,
  harmony,
  skinView,
  pluginView,
  hasCommunitySkins,
  hasCommunityCandidateSkins,
  hasCommunityPlugins,
}: UseSettingsCommunityViewsOptions) {
  const skinTabs = useSettingsTabs(
    skinGridTabs.filter((tab) => tab.id !== "community" || hasCommunitySkins),
    "皮肤分类",
    harmony && mobile,
  );
  const phoneSkinCommunityShown =
    page === "skin" && hasCommunitySkins && skinTabs.selected === "community";
  const skinCommunityShown =
    (page === "skin" && hasCommunityCandidateSkins && skinView === "community") ||
    phoneSkinCommunityShown;
  const pluginCommunityShown =
    page === "plugins" && hasCommunityPlugins && pluginView === "community";

  return { skinTabs, phoneSkinCommunityShown, skinCommunityShown, pluginCommunityShown } as const;
}
