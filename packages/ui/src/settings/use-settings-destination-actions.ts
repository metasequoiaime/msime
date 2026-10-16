import type { Dispatch, SetStateAction } from "react";
import type { AccountCommunityDestination } from "../account/account-model";
import {
  communityDestinationView,
  type CommunityDestinationView,
} from "../community/community-destination";

export interface UseSettingsDestinationActionsOptions {
  // Only the pages these actions open, so any settings page list that has them can pass its own selector.
  selectPage: (page: "skin" | "community" | "about") => void;
  setShowTouchSkinEditor: Dispatch<SetStateAction<boolean>>;
  setCommunityDestination: Dispatch<SetStateAction<AccountCommunityDestination | "all">>;
  /** 触屏宿主没有「社区」页：给了它时，去社区的入口改为在对应功能页里打开完整图库（见 `useCommunityGallerySubview`）。桌面宿主不给，仍去「社区」页。 */
  openCommunityGallery?: (view: CommunityDestinationView) => void;
}

/** Provides the cross-page actions used by home/account/community shortcuts. */
export function useSettingsDestinationActions({
  selectPage,
  setShowTouchSkinEditor,
  setCommunityDestination,
  openCommunityGallery,
}: UseSettingsDestinationActionsOptions) {
  // Local keyboard designs are part of the theme model, so they live on the 皮肤 page.
  const openLocalDesigns = () => {
    selectPage("skin");
    setShowTouchSkinEditor(true);
  };

  const openCommunity = (destination: AccountCommunityDestination | "all") => {
    if (openCommunityGallery) {
      openCommunityGallery(communityDestinationView(destination));
      return;
    }
    selectPage("community");
    setCommunityDestination(destination);
  };

  return {
    openAbout: () => selectPage("about"),
    openCommunity,
    openCommunityAll: () => openCommunity("all"),
    openLocalDesigns,
  } as const;
}
