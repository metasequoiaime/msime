import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../index";
import type { HelpcodeSettings } from "./pages/helpcode-page";

export interface CreateHelpcodeSettingsActionsOptions {
  draft?: Preferences;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates the draft patch callback used by the helper-code settings page. */
export function createHelpcodeSettingsActions({
  draft,
  setDraft,
}: CreateHelpcodeSettingsActionsOptions) {
  return {
    onChange: (patch: HelpcodeSettings) => {
      if (draft) setDraft({ ...draft, ...patch });
    },
  } as const;
}
