import { GroupList, Row } from "../core/platform-controls";
import { ASR_PROVIDER_OPTIONS } from "../voice/voice-provider-options";
import { VoiceLanguageOptions } from "../voice/voice-language-options";
import { VoiceProviderSelect } from "./voice-provider-select";
import { SwitchRow } from "./switch-row";
import { TextInputRow } from "./text-input-row";

export interface VoiceInputCoreSectionProps {
  enabled: boolean;
  provider: string;
  language: string;
  showProviderSettings: boolean;
  systemVoice: boolean;
  macos: boolean;
  harmony: boolean;
  android: boolean;
  localVoiceAvailable: boolean;
  nativeVoicePlatform: boolean;
  harmonyUnsupportedAsr: boolean;
  onEnabledChange: (enabled: boolean) => void;
  onProviderChange: (provider: string) => void;
  onLanguageChange: (language: string) => void;
}

/** Shared voice enablement, provider, and language controls: the 语音输入 page's 识别 group. */
export function VoiceInputCoreSection({
  enabled,
  provider,
  language,
  showProviderSettings,
  systemVoice,
  macos,
  harmony,
  android,
  localVoiceAvailable,
  nativeVoicePlatform,
  harmonyUnsupportedAsr,
  onEnabledChange,
  onProviderChange,
  onLanguageChange,
}: VoiceInputCoreSectionProps) {
  return (
    <GroupList title="识别">
      <SwitchRow
        title="语音输入"
        description="使用语音识别将录音转换为文字"
        aria-label="启用语音输入"
        checked={enabled}
        onChange={onEnabledChange}
      />
      {showProviderSettings && (
        <Row title="识别服务">
          <VoiceProviderSelect
            options={ASR_PROVIDER_OPTIONS}
            ariaLabel="识别服务"
            value={provider}
            onChange={onProviderChange}
          >
            {macos && <option value="system">macOS 系统识别</option>}
            {localVoiceAvailable && <option value="local">本地模型（离线）</option>}
            {!localVoiceAvailable && provider === "local" && (
              <option value="local" disabled>
                本地模型（当前平台不可用）
              </option>
            )}
            {harmony && <option value="system">HarmonyOS 系统识别</option>}
            {android && <option value="system">Android 系统识别</option>}
            {!nativeVoicePlatform && provider === "system" && (
              <option value="system" disabled>
                系统识别（当前平台不可用）
              </option>
            )}
            {harmonyUnsupportedAsr && (
              <option value={provider} disabled>
                {provider}（当前 HarmonyOS 版本不可用）
              </option>
            )}
          </VoiceProviderSelect>
        </Row>
      )}
      <TextInputRow
        title="识别语言"
        description={
          systemVoice ? "选择明确的语言代码，例如 zh-CN、en-US；可用语言由系统决定" : undefined
        }
        label="识别语言"
        maxLength={64}
        list="settings-voice-language-options"
        value={language}
        onChange={onLanguageChange}
      >
        <datalist id="settings-voice-language-options">
          <VoiceLanguageOptions systemVoice={systemVoice} />
        </datalist>
      </TextInputRow>
    </GroupList>
  );
}
