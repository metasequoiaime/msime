import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";
import type { HelpcodeSettings } from "./pages/helpcode-page";
import { createSettingsDraftActions } from "./settings-draft-actions";

export interface CreateHelpcodeSettingsActionsOptions {
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates the draft patch callback used by the helper-code settings page. */
export function createHelpcodeSettingsActions({ setDraft }: CreateHelpcodeSettingsActionsOptions) {
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return {
    onChange: (patch: HelpcodeSettings) => onPreferencesChange(patch),
  } as const;
}
