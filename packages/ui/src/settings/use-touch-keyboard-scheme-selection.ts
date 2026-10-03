import type { Dispatch, SetStateAction } from "react";
import {
  inferredTouchKeyboardScheme,
  selectTouchKeyboardScheme,
  selectHomeTouchKeyboardScheme,
  updateTouchKeyboardSchemeEnabled,
  type TouchKeyboardScheme,
} from "./touch-keyboard-scheme-helpers";
import type { InputScheme, Preferences } from "../index";

export interface UseTouchKeyboardSchemeSelectionOptions {
  draft: Preferences | undefined;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  /** 手写写进偏好的方案：运行中版本的默认方案，缺省是全拼（见 `inferredTouchKeyboardScheme`）。 */
  handwritingScheme?: InputScheme;
}

/** Coordinates touch-keyboard scheme toggles while keeping one selected scheme enabled. */
export function useTouchKeyboardSchemeSelection({
  draft,
  setDraft,
  handwritingScheme = "quanpin",
}: UseTouchKeyboardSchemeSelectionOptions) {
  const selected = draft ? inferredTouchKeyboardScheme(draft, handwritingScheme) : "quanpin";

  const setEnabled = (scheme: TouchKeyboardScheme, enabled: boolean) => {
    if (!draft) return;
    setDraft((current) => {
      if (!current) return current;
      return (
        updateTouchKeyboardSchemeEnabled(
          current,
          scheme,
          enabled,
          inferredTouchKeyboardScheme(current, handwritingScheme),
          handwritingScheme,
        ) ?? current
      );
    });
  };

  const selectHome = (scheme: TouchKeyboardScheme) => {
    if (!draft) return;
    setDraft((current) =>
      current ? selectHomeTouchKeyboardScheme(current, scheme, handwritingScheme) : current,
    );
  };

  const select = (scheme: TouchKeyboardScheme) => {
    if (!draft) return;
    setDraft((current) =>
      current ? selectTouchKeyboardScheme(current, scheme, handwritingScheme) : current,
    );
  };

  return { select, selectHome, selected, setEnabled } as const;
}
