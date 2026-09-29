import { defaultVoiceInput } from "./voice-input-defaults";
import type { VoiceInputPreferences } from "../index";

export interface UseVoiceInputSettingsOptions {
  preferences?: VoiceInputPreferences;
  macos: boolean;
  android: boolean;
  harmony: boolean;
  nativeVoicePlatform: boolean;
  localModelsAvailable: boolean;
  onChange: (patch: Partial<VoiceInputPreferences>) => void;
}

/** Derives voice provider visibility and owns updates to the nested voice preferences. */
export function useVoiceInputSettings({
  preferences,
  macos,
  android,
  harmony,
  nativeVoicePlatform,
  localModelsAvailable,
  onChange,
}: UseVoiceInputSettingsOptions) {
  const voiceInput = { ...defaultVoiceInput, ...preferences };
  const systemVoice = nativeVoicePlatform && voiceInput.asr_provider === "system";
  const systemVoiceHostName = harmony ? "HarmonyOS" : android ? "Android" : "macOS";
  const localVoiceAvailable = macos || localModelsAvailable;
  const localVoice = localVoiceAvailable && voiceInput.asr_provider === "local";
  const serviceVoice = !systemVoice && !localVoice;
  const harmonyUnsupportedAsr =
    harmony &&
    !["doubao", "system", "openai", "siliconflow", "groq", "everyapi", "mistral"].includes(
      String(voiceInput.asr_provider),
    );
  const doubaoAuthMode =
    voiceInput.doubao_auth_mode ||
    (voiceInput.asr_app_key && !voiceInput.asr_app_key.startsWith("<") ? "legacy" : "api_key");
  const updateVoice = (patch: Partial<VoiceInputPreferences>) => onChange(patch);

  return {
    voiceInput,
    systemVoice,
    systemVoiceHostName,
    localVoiceAvailable,
    localVoice,
    serviceVoice,
    harmonyUnsupportedAsr,
    doubaoAuthMode,
    updateVoice,
  };
}
