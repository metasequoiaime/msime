import type { SettingsPageId } from "./settings-page-registry";
import { desktopSkinTabs, type DesktopSkinTab } from "./desktop-skin-tabs";
import { skinGridTabs } from "./skin-grid";
import type { SettingsShellView } from "./use-settings-shell-state";
import { useSettingsTabs } from "./use-settings-tabs";

export interface UseSettingsCommunityViewsOptions {
  page: SettingsPageId;
  mobile: boolean;
  harmony: boolean;
  skinView: DesktopSkinTab;
  onSkinViewChange: (view: DesktopSkinTab) => void;
  pluginView: SettingsShellView;
  hasCommunitySkins: boolean;
  hasCommunityCandidateSkins: boolean;
  hasCommunityPlugins: boolean;
  /** 宿主能装编辑器做的皮肤（`HostCapabilities.candidate_skin_editor` 且客户端有 `candidateSkinEditor`）：桌面皮肤页才有「制作皮肤」标签。 */
  hasCandidateSkinEditor?: boolean;
}

/** 管理设置外壳中的皮肤分类标签，以及皮肤和插件社区图库对表单的覆盖状态。 */
export function useSettingsCommunityViews({
  page,
  mobile,
  harmony,
  skinView,
  onSkinViewChange,
  pluginView,
  hasCommunitySkins,
  hasCommunityCandidateSkins,
  hasCommunityPlugins,
  hasCandidateSkinEditor = false,
}: UseSettingsCommunityViewsOptions) {
  const desktopTabs = useSettingsTabs(
    desktopSkinTabs.filter(
      (tab) =>
        (tab.id !== "community" || hasCommunityCandidateSkins) &&
        (tab.id !== "create" || hasCandidateSkinEditor),
    ),
    "皮肤来源",
    !mobile,
    skinView,
    onSkinViewChange,
  );
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
  // 编辑器和社区图库一样画在偏好表单之外，选中它时表单里的皮肤页隐藏。
  const skinEditorShown =
    page === "skin" && !mobile && hasCandidateSkinEditor && desktopTabs.selected === "create";
  const pluginCommunityShown =
    page === "plugins" && hasCommunityPlugins && pluginView === "community";

  return {
    skinTabs,
    desktopTabs,
    phoneSkinCommunityShown,
    skinCommunityShown,
    skinEditorShown,
    pluginCommunityShown,
  } as const;
}
