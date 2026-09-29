import type { Dispatch, SetStateAction } from "react";
import type { AccountCommunityDestination } from "../account/account-page";
import type { SettingsPageId } from "./mobile-navigation";

export interface UseSettingsDestinationActionsOptions {
  selectPage: (page: SettingsPageId) => void;
  setShowTouchSkinEditor: Dispatch<SetStateAction<boolean>>;
  setCommunityDestination: Dispatch<SetStateAction<AccountCommunityDestination | "all">>;
}

/** Provides the cross-page actions used by home/account/community shortcuts. */
export function useSettingsDestinationActions({
  selectPage,
  setShowTouchSkinEditor,
  setCommunityDestination,
}: UseSettingsDestinationActionsOptions) {
  const openLocalDesigns = () => {
    selectPage("appearance");
    setShowTouchSkinEditor(true);
  };

  const openCommunity = (destination: AccountCommunityDestination | "all") => {
    selectPage("community");
    setCommunityDestination(destination);
  };

  return { openCommunity, openLocalDesigns } as const;
}
