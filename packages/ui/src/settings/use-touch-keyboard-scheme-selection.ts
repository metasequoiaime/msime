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
    const next = updateTouchKeyboardSchemeEnabled(draft, scheme, enabled, selected);
    if (next) setDraft(next);
  };

  const selectHome = (scheme: TouchKeyboardScheme) => {
    if (!draft) return;
    setDraft(selectHomeTouchKeyboardScheme(draft, scheme));
  };

  const select = (scheme: TouchKeyboardScheme) => {
    if (!draft) return;
    setDraft(selectTouchKeyboardScheme(draft, scheme));
  };

  return { select, selectHome, selected, setEnabled } as const;
}
