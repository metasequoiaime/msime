import type { Dispatch, SetStateAction } from "react";
import type { FloatingToolbarPreferences, Preferences } from "../index";

export interface CreateFloatingToolbarSettingsActionsOptions {
  draft?: Preferences;
  floatingToolbar: FloatingToolbarPreferences;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates the nested preference patch callback used by the floating-toolbar settings page. */
export function createFloatingToolbarSettingsActions({
  draft,
  floatingToolbar,
  setDraft,
}: CreateFloatingToolbarSettingsActionsOptions) {
  return {
    onChange: (patch: Partial<FloatingToolbarPreferences>) => {
      if (draft) setDraft({ ...draft, floating_toolbar: { ...floatingToolbar, ...patch } });
    },
  } as const;
}
