import type { Dispatch, SetStateAction } from "react";
import type { MobileKeyboardFeedback } from "./mobile-keyboard-feedback-section";
import type { SettingsSkinPreviewThemes } from "./use-settings-preview-themes";
import type { Preferences } from "../index";

export interface CreateSkinSettingsActionsOptions {
  draft?: Preferences;
  candidatePreviewTheme: "light" | "dark";
  mobileKeyboardFeedback?: MobileKeyboardFeedback;
  saveMobileKeyboardFeedback: (settings: MobileKeyboardFeedback) => Promise<void>;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  setSkinPreviewThemes: Dispatch<SetStateAction<SettingsSkinPreviewThemes>>;
}

/** Creates candidate-skin selection, preview, and mobile palette callbacks. */
export function createSkinSettingsActions({
  draft,
  candidatePreviewTheme,
  mobileKeyboardFeedback,
  saveMobileKeyboardFeedback,
  setDraft,
  setSkinPreviewThemes,
}: CreateSkinSettingsActionsOptions) {
  return {
    onCandidatePaletteChange: (candidatePaletteFollowsDesktop: boolean) => {
      if (mobileKeyboardFeedback) {
        void saveMobileKeyboardFeedback({
          ...mobileKeyboardFeedback,
          candidatePaletteFollowsDesktop,
        });
      }
    },
    onSelect: (id: string) => {
      if (draft) setDraft({ ...draft, candidate_skin: id });
    },
    onTogglePreview: (id: string) =>
      setSkinPreviewThemes((current) => ({
        ...current,
        [id]: (current[id] ?? candidatePreviewTheme) === "dark" ? "light" : "dark",
      })),
  } as const;
}
