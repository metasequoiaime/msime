import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";
import type { LocalModePreferences } from "./local-modes-section";
import { createSettingsDraftActions } from "./settings-draft-actions";

export interface CreateUtilitiesSettingsActionsOptions {
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates the local-mode preference callback used by the utilities settings page. */
export function createUtilitiesSettingsActions({
  setDraft,
}: CreateUtilitiesSettingsActionsOptions) {
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return {
    onLocalModesChange: (local_modes: LocalModePreferences) => onPreferencesChange({ local_modes }),
  } as const;
}
