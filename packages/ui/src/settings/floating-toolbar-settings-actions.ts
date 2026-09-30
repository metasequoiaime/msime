import type { Dispatch, SetStateAction } from "react";
import type { FloatingToolbarPreferences, Preferences } from "../index";
import { floatingToolbarPreferences } from "./floating-toolbar-preferences";

export interface CreateFloatingToolbarSettingsActionsOptions {
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates the nested preference patch callback used by the floating-toolbar settings page. */
export function createFloatingToolbarSettingsActions({
  setDraft,
}: CreateFloatingToolbarSettingsActionsOptions) {
  return {
    onChange: (patch: Partial<FloatingToolbarPreferences>) =>
      setDraft((current) =>
        current
          ? { ...current, floating_toolbar: { ...floatingToolbarPreferences(current), ...patch } }
          : current,
      ),
  } as const;
}
