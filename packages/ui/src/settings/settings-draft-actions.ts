import type { Dispatch, SetStateAction } from "react";
import type { AiAssistantPreferences, Preferences, VoiceInputPreferences } from "../index";
import { defaultAiAssistant } from "./ai-assistant-defaults";
import { defaultVoiceInput } from "./voice-input-defaults";

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
