import type { Preferences } from "../index";

export type ClipboardHistoryPreferencesSource = Pick<Preferences, "clipboard_history">;

/** Resolves the effective clipboard-history state for the settings controls. */
export function clipboardHistoryEnabled(
  ios: boolean,
  draft?: ClipboardHistoryPreferencesSource,
): boolean {
  return ios || (draft?.clipboard_history ?? false);
}
