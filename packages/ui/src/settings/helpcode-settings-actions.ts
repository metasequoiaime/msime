import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../core/preferences-model";
import type { HelpcodeSettings } from "./helpcode-preferences";
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
