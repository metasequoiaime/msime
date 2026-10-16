import type { VoiceInputPreferences } from "./voice-input-preferences";

export { defaultVoiceInput } from "./voice-input-preferences";

/** Returns whether either persisted voice polish preference enables polishing. */
export function isVoicePolishEnabled(
  voiceInput: Pick<VoiceInputPreferences, "polish_text" | "polish_enabled">,
): boolean {
  return voiceInput.polish_text === true || voiceInput.polish_enabled === true;
}

/** Returns the credential label used by the selected ASR provider and auth mode. */
export function voiceAsrTokenLabel(
  provider: string | undefined,
  authMode: "api_key" | "legacy",
): string {
  return provider === "doubao" && authMode !== "legacy" ? "Doubao API Key" : "识别 API Token";
}
