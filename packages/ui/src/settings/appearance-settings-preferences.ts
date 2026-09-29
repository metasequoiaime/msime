import type { Preferences } from "../index";

/** Applies host-specific defaults before preferences reach the appearance settings panel. */
export function appearanceSettingsPreferences(draft: Preferences, windows: boolean): Preferences {
  return {
    ...draft,
    candidate_english_font: windows
      ? (draft.candidate_english_font ?? "Segoe UI")
      : draft.candidate_english_font,
  };
}
