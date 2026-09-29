import type { Dispatch, SetStateAction } from "react";
import type { AccountCommunityDestination } from "../account/account-page";

export interface UseSettingsDestinationActionsOptions {
  // Only the pages these actions open, so any settings page list that has them can pass its own selector.
  selectPage: (page: "skin" | "community") => void;
  setShowTouchSkinEditor: Dispatch<SetStateAction<boolean>>;
  setCommunityDestination: Dispatch<SetStateAction<AccountCommunityDestination | "all">>;
}

/** Provides the cross-page actions used by home/account/community shortcuts. */
export function useSettingsDestinationActions({
  selectPage,
  setShowTouchSkinEditor,
  setCommunityDestination,
}: UseSettingsDestinationActionsOptions) {
  // Local keyboard designs are part of the theme model, so they live on the 皮肤 page.
  const openLocalDesigns = () => {
    selectPage("skin");
    setShowTouchSkinEditor(true);
  };

  const openCommunity = (destination: AccountCommunityDestination | "all") => {
    selectPage("community");
    setCommunityDestination(destination);
  };

  return { openCommunity, openLocalDesigns } as const;
}
