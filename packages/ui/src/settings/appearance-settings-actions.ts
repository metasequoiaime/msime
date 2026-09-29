import type { Dispatch, SetStateAction } from "react";
import type { MobileKeyboardFeedback } from "./mobile-keyboard-feedback-section";
import type { Preferences } from "../index";

export interface CreateAppearanceSettingsActionsOptions {
  draft?: Preferences;
  mobileKeyboardFeedback?: MobileKeyboardFeedback;
  saveMobileKeyboardFeedback: (settings: MobileKeyboardFeedback) => Promise<void>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Creates appearance preference and mobile inline-preedit callbacks. */
export function createAppearanceSettingsActions({
  draft,
  mobileKeyboardFeedback,
  saveMobileKeyboardFeedback,
  setDraft,
}: CreateAppearanceSettingsActionsOptions) {
  return {
    onPreferencesChange: (patch: Partial<Preferences>) => {
      if (draft) setDraft({ ...draft, ...patch });
    },
    onInlinePreeditChange: (inlinePreedit: boolean) => {
      if (mobileKeyboardFeedback) {
        void saveMobileKeyboardFeedback({ ...mobileKeyboardFeedback, inlinePreedit });
      }
    },
  } as const;
}
