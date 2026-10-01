import type { FloatingToolbarPreferences } from "../index";
import { defaultFloatingToolbar } from "./floating-toolbar-defaults";

export type FloatingToolbarPreferencesSource = {
  floating_toolbar?: Partial<FloatingToolbarPreferences>;
};

/** Resolves the effective floating toolbar preferences for settings and previews. */
export function floatingToolbarPreferences(
  draft?: FloatingToolbarPreferencesSource,
): FloatingToolbarPreferences {
  return { ...defaultFloatingToolbar, ...draft?.floating_toolbar };
}
