import type { SettingsPageId } from "./settings-page-registry";
import { communityTabCategories, type CommunityGalleryView } from "./use-community-gallery-subview";
import { useSettingsTabs } from "./use-settings-tabs";

export interface CommunityGalleryNavigationOptions {
  mobile: boolean;
  hasCommunityPage: boolean;
  hasSkins: boolean;
  hasResources: boolean;
  selectPage: (page: SettingsPageId) => void;
  openSubview: (view: CommunityGalleryView) => void;
}

/** 集中管理触屏社区标签的能力投影，以及从功能页入口切到标签或打开完整图库的分流。 */
export function useCommunityGalleryNavigation({
  mobile,
  hasCommunityPage,
  hasSkins,
  hasResources,
  selectPage,
  openSubview,
}: CommunityGalleryNavigationOptions) {
  const tabChoices = communityTabCategories.filter((tab) =>
    tab.id === "skin" ? hasSkins : hasResources,
  );
  const communityTabs = useSettingsTabs(
    tabChoices.length > 0 ? tabChoices : communityTabCategories,
    "社区分类",
    mobile && tabChoices.length > 1,
  );
  const communityTabShown = mobile && hasCommunityPage;

  // 浏览整类皮肤或词库时切到社区标签；「我的」范围和回复模板仍在功能页打开完整图库。
  const openCommunityGallery = (requested: CommunityGalleryView) => {
    const view: CommunityGalleryView =
      requested.category === "skin" && !hasSkins
        ? { category: "dictionary", scope: requested.scope, initialMine: false }
        : requested;
    if (
      communityTabShown &&
      view.category !== "reply" &&
      view.scope === "" &&
      !view.initialMine &&
      tabChoices.some((tab) => tab.id === view.category)
    ) {
      communityTabs.select(view.category);
      selectPage("community");
      return;
    }
    openSubview(view);
  };

  return { communityTabs, openCommunityGallery } as const;
}
