import type { VoiceInputPreferences } from "../index";
import type { DoubaoAuthMode } from "./doubao-auth-mode-section";
import {
  ASR_PROVIDER_DEFAULTS,
  POLISH_PROVIDER_DEFAULTS,
  providerSettingValue,
} from "../voice/voice-providers";

function serviceCredentialTestConfig(
  provider: string | undefined,
  endpoint: string | undefined,
  model: string | undefined,
  token: string | undefined,
  defaults: Record<string, { endpoint: string; model: string }>,
): Record<string, unknown> {
  return {
    provider,
    endpoint: providerSettingValue(endpoint, provider, defaults, "endpoint"),
    model: providerSettingValue(model, provider, defaults, "model"),
    token: token ?? "",
  };
}

/** Configuration sent to the Linux voice provider when checking ASR settings. */
export function asrProviderCredentialTestConfig(
  voiceInput: VoiceInputPreferences,
  doubaoAuthMode: DoubaoAuthMode,
): Record<string, unknown> {
  return {
    asr_provider: voiceInput.asr_provider ?? "doubao",
    asr_model: voiceInput.asr_model ?? "",
    asr_resource_id: voiceInput.asr_resource_id ?? "",
    doubao_auth_mode: doubaoAuthMode,
    doubao_enable_itn: voiceInput.doubao_enable_itn !== false,
    doubao_enable_punc: voiceInput.doubao_enable_punc !== false,
    doubao_enable_ddc: voiceInput.doubao_enable_ddc === true,
  };
}

/** Configuration sent directly to a remote ASR service for a synthetic-silence check. */
export function asrServiceCredentialTestConfig(
  voiceInput: VoiceInputPreferences,
  doubaoAuthMode: DoubaoAuthMode,
): Record<string, unknown> {
  const provider = voiceInput.asr_provider;
  return {
    ...serviceCredentialTestConfig(
      provider,
      voiceInput.asr_endpoint,
      voiceInput.asr_model,
      voiceInput.asr_token,
      ASR_PROVIDER_DEFAULTS,
    ),
    ...(provider === "doubao"
      ? {
          auth_mode: doubaoAuthMode,
          app_id: doubaoAuthMode === "legacy" ? (voiceInput.asr_app_key ?? "") : "",
          resource_id: voiceInput.asr_resource_id ?? "volc.seedasr.sauc.duration",
          doubao_enable_itn: voiceInput.doubao_enable_itn !== false,
          doubao_enable_punc: voiceInput.doubao_enable_punc !== false,
          doubao_enable_ddc: voiceInput.doubao_enable_ddc === true,
          doubao_boosting_table_id: voiceInput.doubao_boosting_table_id ?? "",
        }
      : {}),
  };
}

/** Whether the remote ASR credential test lacks the credentials required by its auth mode. */
export function asrServiceCredentialTestDisabled(
  voiceInput: VoiceInputPreferences,
  doubaoAuthMode: DoubaoAuthMode,
): boolean {
  return (
    !voiceInput.asr_token?.trim() ||
    (voiceInput.asr_provider === "doubao" &&
      doubaoAuthMode === "legacy" &&
      !voiceInput.asr_app_key?.trim())
  );
}

/** Configuration sent to the Linux voice provider when checking polish settings. */
export function polishProviderCredentialTestConfig(
  voiceInput: VoiceInputPreferences,
): Record<string, unknown> {
  return {
    polish_provider: voiceInput.polish_provider ?? "siliconflow",
    polish_model: voiceInput.polish_model ?? "",
  };
}

/** Configuration sent directly to a remote polish service for a credential check. */
export function polishServiceCredentialTestConfig(
  voiceInput: VoiceInputPreferences,
): Record<string, unknown> {
  const provider = voiceInput.polish_provider ?? "siliconflow";
  return serviceCredentialTestConfig(
    provider,
    voiceInput.polish_endpoint,
    voiceInput.polish_model,
    voiceInput.polish_token,
    POLISH_PROVIDER_DEFAULTS,
  );
}

/** Whether the remote polish credential test lacks its API token. */
export function polishServiceCredentialTestDisabled(voiceInput: VoiceInputPreferences): boolean {
  return !voiceInput.polish_token?.trim();
}
