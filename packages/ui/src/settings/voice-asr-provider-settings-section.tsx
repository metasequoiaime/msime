import type { VoiceInputPreferences } from "../index";
import { voiceAsrTokenLabel } from "./voice-input-defaults";
import { ASR_PROVIDER_DEFAULTS } from "../voice/voice-providers";
import { DoubaoAuthModeSection } from "./doubao-auth-mode-section";
import { DoubaoResourceIdSection } from "./doubao-resource-id-section";
import { DoubaoStreamEndpointSection } from "./doubao-stream-endpoint-section";
import { VoiceCredentialFieldsSection } from "./voice-credential-fields-section";
import { VoiceEndpointSection } from "./voice-endpoint-section";
import { VoiceModelSection } from "./voice-model-section";
import type { ProviderPresetControlFactory } from "./provider-preset-control";

export interface VoiceAsrProviderSettingsSectionProps {
  voiceInput: VoiceInputPreferences;
  showProviderSettings: boolean;
  serviceVoice: boolean;
  linux: boolean;
  doubaoAuthMode: "api_key" | "legacy";
  providerPresetControls: ProviderPresetControlFactory;
  providerPresetClassName?: string;
  updateVoice: (patch: Partial<VoiceInputPreferences>) => void;
}

/** Shared ASR provider model, endpoint, credential, and resource settings. */
export function VoiceAsrProviderSettingsSection({
  voiceInput,
  showProviderSettings,
  serviceVoice,
  linux,
  doubaoAuthMode,
  providerPresetControls,
  providerPresetClassName,
  updateVoice,
}: VoiceAsrProviderSettingsSectionProps) {
  const isDoubao = voiceInput.asr_provider === "doubao";
  if (!showProviderSettings || (!serviceVoice && !isDoubao)) return null;

  return (
    <>
      {serviceVoice &&
        providerPresetControls(
          "识别服务",
          ASR_PROVIDER_DEFAULTS[String(voiceInput.asr_provider)],
          voiceInput.asr_model ?? "",
          (asr_model) => updateVoice({ asr_model }),
          providerPresetClassName,
        )}
      {serviceVoice && (
        <VoiceModelSection
          value={voiceInput.asr_model ?? ""}
          onChange={(asr_model) => updateVoice({ asr_model })}
        />
      )}
      {isDoubao && (
        <DoubaoAuthModeSection
          value={doubaoAuthMode}
          linux={linux}
          onChange={(doubao_auth_mode) => updateVoice({ doubao_auth_mode })}
        />
      )}
      {!linux && serviceVoice && (
        <>
          {isDoubao && (
            <DoubaoStreamEndpointSection
              endpoint={voiceInput.asr_endpoint ?? ""}
              onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
            />
          )}
          <VoiceEndpointSection
            value={voiceInput.asr_endpoint ?? ""}
            onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
          />
          <VoiceCredentialFieldsSection
            showAppKey={isDoubao && doubaoAuthMode === "legacy"}
            appKey={voiceInput.asr_app_key ?? ""}
            tokenLabel={voiceAsrTokenLabel(voiceInput.asr_provider, doubaoAuthMode)}
            token={voiceInput.asr_token ?? ""}
            onAppKeyChange={(asr_app_key) => updateVoice({ asr_app_key })}
            onTokenChange={(asr_token) => updateVoice({ asr_token })}
          />
        </>
      )}
      {/* 只有豆包的识别请求读资源 ID（client-core 的 voice provider、Windows 的 VoiceSessionPolicy 和 Linux 语音服务都只在豆包分支读它），换成别的服务时这一行没有作用，所以不显示。 */}
      {serviceVoice && isDoubao && (
        <DoubaoResourceIdSection
          value={voiceInput.asr_resource_id ?? ""}
          onChange={(asr_resource_id) => updateVoice({ asr_resource_id })}
        />
      )}
    </>
  );
}
