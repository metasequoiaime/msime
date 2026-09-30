import type { Dispatch, SetStateAction } from "react";
import type {
  AiAssistantPreferences,
  CustomTheme,
  Preferences,
  VoiceInputPreferences,
} from "../index";
import { defaultAiAssistant } from "./ai-assistant-defaults";
import { defaultVoiceInput } from "./voice-input-defaults";
import { defaultTouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import { updateCustomKeyboard } from "./theme-selection-updates";

export interface CreateSettingsDraftActionsOptions {
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
}

/** Builds the nested draft updates shared by settings hooks. */
export function createSettingsDraftActions({ setDraft }: CreateSettingsDraftActionsOptions) {
  return {
    onPreferencesChange: (patch: Partial<Preferences>) =>
      setDraft((current) => (current ? { ...current, ...patch } : current)),
    onAiChange: (patch: Partial<AiAssistantPreferences>) =>
      setDraft((current) =>
        current
          ? {
              ...current,
              ai_assistant: { ...(current.ai_assistant ?? defaultAiAssistant), ...patch },
            }
          : current,
      ),
    onTranslationChange: (next: Preferences) => setDraft(next),
    onCustomThemeChange: (patch: Partial<CustomTheme>) =>
      setDraft((current) =>
        current ? { ...current, custom_theme: { ...current.custom_theme, ...patch } } : current,
      ),
    onCustomKeyboardChange: (design: TouchKeyboardSkinDesign) =>
      setDraft((current) => (current ? updateCustomKeyboard(current, design) : current)),
    onUseCustomKeyboard: () =>
      setDraft((current) =>
        current
          ? updateCustomKeyboard(
              current,
              current.custom_theme?.keyboard ?? defaultTouchKeyboardSkinDesign,
            )
          : current,
      ),
    onVoiceChange: (patch: Partial<VoiceInputPreferences>) =>
      setDraft((current) =>
        current
          ? {
              ...current,
              voice_input: { ...defaultVoiceInput, ...current.voice_input, ...patch },
            }
          : current,
      ),
  } as const;
}
