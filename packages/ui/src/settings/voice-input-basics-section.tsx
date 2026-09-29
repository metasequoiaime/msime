import type { VoiceInputPreferences } from "../index";
import { asrProviderUpdate } from "../voice/voice-providers";
import { VoiceInputCoreSection } from "./voice-input-core-section";
import { VoiceInputIntroSection } from "./voice-input-intro-section";

export interface VoiceInputBasicsSectionProps {
  localVoice: boolean;
  localVoiceModelsAvailable: boolean;
  systemVoice: boolean;
  systemVoiceHostName: string;
  android: boolean;
  ios: boolean;
  macos: boolean;
  harmony: boolean;
  linux: boolean;
  showVoiceProviderSettings: boolean;
  localVoiceAvailable: boolean;
  nativeVoicePlatform: boolean;
  harmonyUnsupportedAsr: boolean;
  voiceInput: VoiceInputPreferences;
  updateVoice: (patch: Partial<VoiceInputPreferences>) => void;
  onOpenVoice?: () => void;
}

/** Shared voice introduction and recognition enablement/provider controls. */
export function VoiceInputBasicsSection({
  localVoice,
  localVoiceModelsAvailable,
  systemVoice,
  systemVoiceHostName,
  android,
  ios,
  macos,
  harmony,
  linux,
  showVoiceProviderSettings,
  localVoiceAvailable,
  nativeVoicePlatform,
  harmonyUnsupportedAsr,
  voiceInput,
  updateVoice,
  onOpenVoice,
}: VoiceInputBasicsSectionProps) {
  return (
    <>
      <VoiceInputIntroSection
        localVoice={localVoice}
        localVoiceModelsAvailable={localVoiceModelsAvailable}
        systemVoice={systemVoice}
        systemVoiceHostName={systemVoiceHostName}
        android={android}
        ios={ios}
        macos={macos}
        harmony={harmony}
        linux={linux}
        showVoiceProviderSettings={showVoiceProviderSettings}
        onOpenVoice={onOpenVoice}
      />
      <VoiceInputCoreSection
        enabled={voiceInput.enabled}
        provider={String(voiceInput.asr_provider)}
        language={voiceInput.language}
        showProviderSettings={showVoiceProviderSettings}
        systemVoice={systemVoice}
        macos={macos}
        harmony={harmony}
        android={android}
        localVoiceAvailable={localVoiceAvailable}
        nativeVoicePlatform={nativeVoicePlatform}
        harmonyUnsupportedAsr={harmonyUnsupportedAsr}
        onEnabledChange={(enabled) => updateVoice({ enabled })}
        onProviderChange={(provider) =>
          updateVoice({
            ...asrProviderUpdate(provider, voiceInput),
            ...(provider === "system" && voiceInput.language === "auto"
              ? { language: "zh-CN" }
              : {}),
            ...(linux ? { asr_resource_id: "", doubao_boosting_table_id: "" } : {}),
          })
        }
        onLanguageChange={(language) => updateVoice({ language })}
      />
    </>
  );
}
