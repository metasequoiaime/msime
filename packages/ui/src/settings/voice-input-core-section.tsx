import { GroupList, Row, Select, Switch } from "../core/platform-controls";
import { ASR_PROVIDER_OPTIONS, VoiceProviderOptions } from "../voice/voice-provider-options";

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
      <Row title="语音输入" description="使用语音识别将录音转换为文字">
        <Switch aria-label="启用语音输入" checked={enabled} onChange={onEnabledChange} />
      </Row>
      {showProviderSettings && (
        <Row title="识别服务">
          <Select
            aria-label="识别服务"
            value={provider}
            onChange={(event) => onProviderChange(event.target.value)}
          >
            <VoiceProviderOptions options={ASR_PROVIDER_OPTIONS} />
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
          </Select>
        </Row>
      )}
      <Row
        title="识别语言"
        description={
          systemVoice ? "选择明确的语言代码，例如 zh-CN、en-US；可用语言由系统决定" : undefined
        }
      >
        <input
          aria-label="识别语言"
          maxLength={64}
          list="settings-voice-language-options"
          value={language}
          onChange={(event) => onLanguageChange(event.target.value)}
        />
        <datalist id="settings-voice-language-options">
          <option value={systemVoice ? "zh-CN" : "zh-cn"}>中文（普通话）</option>
          <option value={systemVoice ? "en-US" : "en"}>English</option>
          <option value={systemVoice ? "ja-JP" : "ja"}>日本語</option>
          {!systemVoice && <option value="auto">自动识别</option>}
        </datalist>
      </Row>
    </GroupList>
  );
}
