import type { Dispatch, SetStateAction } from "react";
import type { MobileKeyboardFeedback } from "./mobile-keyboard-feedback-section";
import type { Preferences } from "../index";
import { createSettingsDraftActions } from "./settings-draft-actions";

export interface CreateAppearanceSettingsActionsOptions {
  mobileKeyboardFeedback?: MobileKeyboardFeedback;
  saveMobileKeyboardFeedback: (settings: MobileKeyboardFeedback) => Promise<void>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates appearance preference and mobile inline-preedit callbacks. */
export function createAppearanceSettingsActions({
  mobileKeyboardFeedback,
  saveMobileKeyboardFeedback,
  setDraft,
}: CreateAppearanceSettingsActionsOptions) {
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return {
    onPreferencesChange,
    onInlinePreeditChange: (inlinePreedit: boolean) => {
      if (mobileKeyboardFeedback) {
        void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, inlinePreedit });
      }
    },
  } as const;
}
