import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";
import type { LocalModePreferences } from "./local-modes-section";

export interface CreateUtilitiesSettingsActionsOptions {
  draft?: Preferences;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates the local-mode preference callback used by the utilities settings page. */
export function createUtilitiesSettingsActions({
  draft,
  setDraft,
}: CreateUtilitiesSettingsActionsOptions) {
  return {
    onLocalModesChange: (local_modes: LocalModePreferences) => {
      if (draft) setDraft({ ...draft, local_modes });
    },
  } as const;
}
