import type { ReactNode } from "react";
import type { VoiceInputPreferences } from "../index";
import { defaultVoiceInput, voiceAsrTokenLabel } from "./voice-input-defaults";
import { ASR_PROVIDER_DEFAULTS } from "../voice/voice-providers";
import { MoreOptions } from "../core/platform-controls";
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
  /** 把接口地址、识别模型、资源 ID 收进「更多选项」，接入说明、鉴权方式和密钥留在外面。桌面的「服务配置」子页这样排；手机照旧平铺。 */
  foldAdvanced?: boolean;
  /** 收进同一个「更多选项」的其他行，例如豆包的识别选项。只在 `foldAdvanced` 时使用。 */
  advanced?: ReactNode;
}

/** 存的值和出厂值不同（用户改过接口地址或资源 ID）时，「更多选项」一打开就展开，免得改过的值藏起来看不见。 */
function advancedChanged(voiceInput: VoiceInputPreferences): boolean {
  const provider = String(voiceInput.asr_provider ?? "");
  const endpoint = (voiceInput.asr_endpoint ?? "").trim();
  const resourceId = (voiceInput.asr_resource_id ?? "").trim();
  return (
    (endpoint !== "" && endpoint !== (ASR_PROVIDER_DEFAULTS[provider]?.endpoint ?? "")) ||
    (provider === "doubao" && resourceId !== "" && resourceId !== defaultVoiceInput.asr_resource_id)
  );
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
  foldAdvanced = false,
  advanced,
}: VoiceAsrProviderSettingsSectionProps) {
  const isDoubao = voiceInput.asr_provider === "doubao";
  if (!showProviderSettings || (!serviceVoice && !isDoubao)) return null;

  // 豆包没有可选的识别模型，它的识别请求也不读 `asr_model`（macOS 的 DoubaoVoiceRequest 只取接口、密钥、App Key、资源 ID、鉴权方式和热词表），所以选豆包时不画这一行。
  const model = serviceVoice && !isDoubao && (
    <VoiceModelSection
      models={ASR_PROVIDER_DEFAULTS[String(voiceInput.asr_provider)]?.models ?? []}
      value={voiceInput.asr_model ?? ""}
      onChange={(asr_model) => updateVoice({ asr_model })}
    />
  );
  // 豆包的两个流式预设和自定义地址写的都是 `asr_endpoint`，合成一个控件；其他服务只有地址输入框。
  const endpoint =
    !linux &&
    serviceVoice &&
    (isDoubao ? (
      <DoubaoStreamEndpointSection
        endpoint={voiceInput.asr_endpoint ?? ""}
        onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
      />
    ) : (
      <VoiceEndpointSection
        value={voiceInput.asr_endpoint ?? ""}
        onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
      />
    ));
  const credentials = !linux && serviceVoice && (
    <VoiceCredentialFieldsSection
      showAppKey={isDoubao && doubaoAuthMode === "legacy"}
      appKey={voiceInput.asr_app_key ?? ""}
      tokenLabel={voiceAsrTokenLabel(voiceInput.asr_provider, doubaoAuthMode)}
      token={voiceInput.asr_token ?? ""}
      onAppKeyChange={(asr_app_key) => updateVoice({ asr_app_key })}
      onTokenChange={(asr_token) => updateVoice({ asr_token })}
    />
  );
  // 只有豆包的识别请求读资源 ID（client-providers 的 voice provider、Windows 的 VoiceSessionPolicy 和 Linux 语音服务都只在豆包分支读它），换成别的服务时这一行没有作用，所以不显示。
  const resourceId = serviceVoice && isDoubao && (
    <DoubaoResourceIdSection
      value={voiceInput.asr_resource_id ?? ""}
      onChange={(asr_resource_id) => updateVoice({ asr_resource_id })}
    />
  );
  const authMode = isDoubao && (
    <DoubaoAuthModeSection
      value={doubaoAuthMode}
      linux={linux}
      onChange={(doubao_auth_mode) => updateVoice({ doubao_auth_mode })}
    />
  );
  const documentation =
    serviceVoice &&
    providerPresetControls(
      "识别服务",
      ASR_PROVIDER_DEFAULTS[String(voiceInput.asr_provider)],
      providerPresetClassName,
    );

  if (foldAdvanced)
    return (
      <>
        {documentation}
        {authMode}
        {credentials}
        {(endpoint || model || resourceId || advanced) && (
          <MoreOptions defaultOpen={advancedChanged(voiceInput)}>
            {endpoint}
            {model}
            {resourceId}
            {advanced}
          </MoreOptions>
        )}
      </>
    );
  return (
    <>
      {documentation}
      {model}
      {authMode}
      {endpoint}
      {credentials}
      {resourceId}
    </>
  );
}
