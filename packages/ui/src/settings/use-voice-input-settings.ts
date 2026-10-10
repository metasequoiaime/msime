import { defaultVoiceInput } from "./voice-input-defaults";
import type { VoiceInputPreferences } from "../index";

export interface UseVoiceInputSettingsOptions {
  preferences?: VoiceInputPreferences;
  macos: boolean;
  android: boolean;
  harmony: boolean;
  /** Windows 的系统识别器叫「Windows 系统识别」。 */
  windows?: boolean;
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
  windows = false,
  nativeVoicePlatform,
  localModelsAvailable,
  onChange,
}: UseVoiceInputSettingsOptions) {
  const voiceInput = { ...defaultVoiceInput, ...preferences };
  const systemVoice = nativeVoicePlatform && voiceInput.asr_provider === "system";
  const systemVoiceHostName = harmony
    ? "HarmonyOS"
    : android
      ? "Android"
      : windows
        ? "Windows"
        : "macOS";
  const localVoiceAvailable = macos || localModelsAvailable;
  const localVoice = localVoiceAvailable && voiceInput.asr_provider === "local";
  const serviceVoice = !systemVoice && !localVoice;
  const harmonyUnsupportedAsr =
    harmony &&
    ![
      "doubao",
      "system",
      "openai",
      "siliconflow",
      "groq",
      "everyapi",
      "mistral",
      "bailian",
    ].includes(String(voiceInput.asr_provider));
  const doubaoAuthMode = voiceInput.doubao_auth_mode || "api_key";
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
