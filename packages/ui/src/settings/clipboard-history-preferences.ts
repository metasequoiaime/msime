import type { Preferences } from "../core/preferences-model";

export type ClipboardHistoryPreferencesSource = Pick<Preferences, "clipboard_history">;

export const clipboardHistoryLimits = [50, 100, 200, 500, 1000] as const;
export const defaultClipboardHistoryLimit = 50;

export function clipboardHistoryLimit(
  preferences?: Pick<Preferences, "clipboard_history_limit">,
): number {
  const value = preferences?.clipboard_history_limit;
  return clipboardHistoryLimits.includes(value as (typeof clipboardHistoryLimits)[number])
    ? (value as number)
    : defaultClipboardHistoryLimit;
}

/** Resolves the effective clipboard-history state for the settings controls. */
export function clipboardHistoryEnabled(
  ios: boolean,
  draft?: ClipboardHistoryPreferencesSource,
): boolean {
  return ios || (draft?.clipboard_history ?? false);
}
