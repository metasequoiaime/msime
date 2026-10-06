import type { ReactNode } from "react";
import type { VoiceInputPreferences } from "../index";
import { POLISH_PROVIDER_DEFAULTS, polishProviderUpdate } from "../voice/voice-providers";
import { isVoicePolishEnabled } from "./voice-input-defaults";
import { PolishCredentialFieldsSection } from "./polish-credential-fields-section";
import { PolishPromptSection } from "./polish-prompt-section";
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
      providerPreset={providerPresetControls(
        "文本润色",
        POLISH_PROVIDER_DEFAULTS[voiceInput.polish_provider ?? "siliconflow"],
        voiceInput.polish_model ?? "",
        (polish_model) => updateVoice({ polish_model }),
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
