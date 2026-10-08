import type { VoiceInputPreferences } from "../index";
import { utf8ByteLength } from "../core/text";
import type { DoubaoAuthMode } from "./doubao-auth-mode-section";
import {
  ASR_PROVIDER_DEFAULTS,
  POLISH_PROVIDER_DEFAULTS,
  providerSettingValue,
} from "../voice/voice-providers";

// 与偏好设置保存校验和 runtime options（`voice.rs` 的 `voice_provider_options`）的模型路径上限一致：超过上限的路径不发，由 provider 报模型不可用。
const MAX_MODEL_PATH_BYTES = 4096;

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
  const modelPath = voiceInput.asr_provider === "local" ? voiceInput.asr_model_path : undefined;
  return {
    asr_provider: voiceInput.asr_provider ?? "doubao",
    asr_model: voiceInput.asr_model ?? "",
    asr_resource_id: voiceInput.asr_resource_id ?? "",
    doubao_auth_mode: doubaoAuthMode,
    doubao_enable_itn: voiceInput.doubao_enable_itn !== false,
    doubao_enable_punc: voiceInput.doubao_enable_punc !== false,
    doubao_enable_ddc: voiceInput.doubao_enable_ddc === true,
    // 本地识别没有凭据可测，provider 校验的是这个模型目录；不带上它，测试必然报模型不可用。
    ...(modelPath && utf8ByteLength(modelPath) <= MAX_MODEL_PATH_BYTES
      ? { asr_model_path: modelPath }
      : {}),
  };
}

/** Linux 语音识别测试是否无从测起：选了本地识别却还没有选模型。 */
export function asrProviderCredentialTestDisabled(voiceInput: VoiceInputPreferences): boolean {
  return voiceInput.asr_provider === "local" && !voiceInput.asr_model_path;
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
