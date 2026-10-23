import type { ReactNode } from "react";
import type { VoiceInputPreferences } from "../index";
import { POLISH_PROVIDER_DEFAULTS, polishProviderUpdate } from "../voice/voice-providers";
import { isVoicePolishEnabled } from "./voice-input-defaults";
import { PolishCredentialFieldsSection } from "./polish-credential-fields-section";
import { PolishPromptSection } from "./polish-prompt-section";
import { SelectRow } from "./select-row";
import type { ProviderPresetControlFactory } from "./provider-preset-control";
import { VoicePolishSection } from "./voice-polish-section";
import * as settings from "./settings-style";

export interface VoicePolishSettingsSectionProps {
  voiceInput: VoiceInputPreferences;
  linux: boolean;
  providerPresetControls: ProviderPresetControlFactory;
  updateVoice: (patch: Partial<VoiceInputPreferences>) => void;
  /** Linux 的润色凭据由语音服务保管，不在共享设置里；传入时放在接口地址与令牌的位置，也就是方案与提示词之前。 */
  linuxCredentials?: ReactNode;
  children?: ReactNode;
}

/** Shared voice polishing settings, including credentials and prompt slots. */
export function VoicePolishSettingsSection({
  voiceInput,
  linux,
  providerPresetControls,
  updateVoice,
  linuxCredentials,
  children,
}: VoicePolishSettingsSectionProps) {
  return (
    <VoicePolishSection
      enabled={isVoicePolishEnabled(voiceInput)}
      provider={voiceInput.polish_provider ?? "siliconflow"}
      model={voiceInput.polish_model ?? ""}
      presetModels={
        POLISH_PROVIDER_DEFAULTS[voiceInput.polish_provider ?? "siliconflow"]?.models ?? []
      }
      providerPreset={providerPresetControls(
        "文本润色",
        POLISH_PROVIDER_DEFAULTS[voiceInput.polish_provider ?? "siliconflow"],
        settings.managerBlock,
      )}
      onEnabledChange={(enabled) => updateVoice({ polish_text: enabled, polish_enabled: enabled })}
      onProviderChange={(provider) => updateVoice(polishProviderUpdate(provider, voiceInput))}
      onModelChange={(polish_model) => updateVoice({ polish_model })}
    >
      {linux ? (
        linuxCredentials
      ) : (
        <PolishCredentialFieldsSection
          endpoint={voiceInput.polish_endpoint ?? ""}
          token={voiceInput.polish_token ?? ""}
          onEndpointChange={(polish_endpoint) => updateVoice({ polish_endpoint })}
          onTokenChange={(polish_token) => updateVoice({ polish_token })}
        />
      )}
      <SelectRow
        title="思考强度"
        description="仅 DeepSeek 生效。开启后润色更细致，但上屏前要多等几秒到几十秒，超过 30 秒直接上屏原文。"
        hidden={(voiceInput.polish_provider ?? "siliconflow") !== "deepseek"}
        value={voiceInput.polish_thinking ?? "disabled"}
        aria-label="润色思考强度"
        onChange={(event) => updateVoice({ polish_thinking: event.currentTarget.value })}
      >
        <option value="disabled">关闭</option>
        <option value="low">低</option>
        <option value="high">高</option>
        <option value="max">最高</option>
      </SelectRow>
      <PolishPromptSection
        promptId={voiceInput.polish_prompt_id}
        customPrompts={{
          custom_1: voiceInput.polish_prompt_custom_1,
          custom_2: voiceInput.polish_prompt_custom_2,
          custom_3: voiceInput.polish_prompt_custom_3,
        }}
        onSelectPrompt={(polish_prompt_id) => updateVoice({ polish_prompt_id })}
        onCustomPromptChange={(slot, prompt) => updateVoice({ [`polish_prompt_${slot}`]: prompt })}
        actions={children}
      />
    </VoicePolishSection>
  );
}
