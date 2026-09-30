import type { Dispatch, SetStateAction } from "react";
import {
  inferredTouchKeyboardScheme,
  selectTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  type TouchKeyboardScheme,
} from "./touch-keyboard-scheme-helpers";
import type { Preferences } from "../index";

export interface UseTouchKeyboardSchemeSelectionOptions {
  draft: Preferences | undefined;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Coordinates touch-keyboard scheme toggles while keeping one selected scheme enabled. */
export function useTouchKeyboardSchemeSelection({
  draft,
  setDraft,
}: UseTouchKeyboardSchemeSelectionOptions) {
  const selected = draft ? inferredTouchKeyboardScheme(draft) : "quanpin";

  const setEnabled = (scheme: TouchKeyboardScheme, enabled: boolean) => {
    if (!draft) return;
    setDraft((current) => {
      if (!current) return current;
      return (
        updateTouchKeyboardSchemeEnabled(
          current,
          scheme,
          enabled,
          inferredTouchKeyboardScheme(current),
        ) ?? current
      );
    });
  };

  const selectHome = (scheme: TouchKeyboardScheme) => {
    if (!draft) return;
    setDraft((current) => (current ? selectHomeTouchKeyboardScheme(current, scheme) : current));
  };

  const select = (scheme: TouchKeyboardScheme) => {
    if (!draft) return;
    setDraft((current) => (current ? selectTouchKeyboardScheme(current, scheme) : current));
  };

  return { select, selectHome, selected, setEnabled } as const;
}
