import type { Dispatch, SetStateAction } from "react";
import type { Preferences } from "../core/preferences-model";
import type { FloatingToolbarPreferences } from "./floating-toolbar-preferences";
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
