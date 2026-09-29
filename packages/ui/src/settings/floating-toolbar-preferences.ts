import type { FloatingToolbarPreferences, Preferences } from "../index";
import { defaultFloatingToolbar } from "./floating-toolbar-defaults";

export type FloatingToolbarPreferencesSource = Pick<Preferences, "floating_toolbar">;

/** Resolves the effective floating toolbar preferences for settings and previews. */
export function floatingToolbarPreferences(
  draft?: FloatingToolbarPreferencesSource,
): FloatingToolbarPreferences {
  return { ...defaultFloatingToolbar, ...draft?.floating_toolbar };
}
