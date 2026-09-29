import type { VoiceInputPreferences } from "../index";

/** Mirrors the default voice provider and resource used by client-core. */
export const defaultVoiceInput: VoiceInputPreferences = {
  enabled: true,
  language: "zh-CN",
  asr_provider: "doubao",
  doubao_auth_mode: "api_key",
  asr_resource_id: "volc.seedasr.sauc.duration",
};

/** Returns whether either persisted voice polish preference enables polishing. */
export function isVoicePolishEnabled(
  voiceInput: Pick<VoiceInputPreferences, "polish_text" | "polish_enabled">,
): boolean {
  return voiceInput.polish_text === true || voiceInput.polish_enabled === true;
}
