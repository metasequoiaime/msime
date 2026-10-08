import { GroupList } from "../core/platform-controls";
import { ASR_PROVIDER_OPTIONS } from "../voice/voice-provider-options";
import {
  MOBILE_VOICE_LANGUAGES,
  VoiceLanguageOptions,
  mobileVoiceLanguageOf,
  mobileVoiceLanguageSupported,
  mobileVoiceLanguageValue,
} from "../voice/voice-language-options";
import { VoiceProviderRow } from "./voice-provider-row";
import { SelectRow } from "./select-row";
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

export type VoiceInputServiceRowsProps = Omit<
  VoiceInputCoreSectionProps,
  "language" | "systemVoice" | "onLanguageChange"
>;

/** 语音输入总开关和识别服务选择，不带分组：大多数宿主上由识别分组容纳它们，HarmonyOS 手机上由识别服务分组容纳。 */
export function VoiceInputServiceRows({
  enabled,
  provider,
  showProviderSettings,
  macos,
  harmony,
  android,
  localVoiceAvailable,
  nativeVoicePlatform,
  harmonyUnsupportedAsr,
  onEnabledChange,
  onProviderChange,
}: VoiceInputServiceRowsProps) {
  return (
    <>
      <SwitchRow
        title="语音输入"
        description="使用语音识别将录音转换为文字"
        aria-label="启用语音输入"
        checked={enabled}
        onChange={onEnabledChange}
      />
      {showProviderSettings && (
        <VoiceProviderRow
          title="识别服务"
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
        </VoiceProviderRow>
      )}
    </>
  );
}

export interface MobileVoiceLanguageRowProps {
  /** 存储的 `voice_input.language`。 */
  language: string;
  /** 选中的是系统识别器：语言以 locale 形式传给它，且它只识别普通话。 */
  systemVoice: boolean;
  onLanguageChange: (language: string) => void;
}

/** 手机语音页的识别语言：四个具名选项而不是自由填写的语言代码，与 Android 的 VoicePage 提供的一致。 */
export function MobileVoiceLanguageRow({
  language,
  systemVoice,
  onLanguageChange,
}: MobileVoiceLanguageRowProps) {
  const known = mobileVoiceLanguageOf(language);
  return (
    <SelectRow
      title="识别语言"
      description={systemVoice ? "系统识别只支持普通话" : undefined}
      aria-label="识别语言"
      value={known ?? language}
      onChange={(event) => {
        const next = mobileVoiceLanguageOf(event.target.value);
        // 选择未知存储值的原样选项不改变任何东西。
        if (next !== null && next !== known)
          onLanguageChange(mobileVoiceLanguageValue(next, systemVoice));
      }}
    >
      {/* 这里直接写成 `<option>` 而不通过组件，因为 HarmonyOS 面板从 select 的直接子元素读取选项。识别器无法支持的选项被禁用；不属于这四个的存储值保留一个自己的选项，以便这一行能显示它。 */}
      {MOBILE_VOICE_LANGUAGES.map((choice) => (
        <option
          key={choice.value}
          value={choice.value}
          disabled={!mobileVoiceLanguageSupported(choice.value, systemVoice)}
        >
          {choice.label}
        </option>
      ))}
      {known === null && <option value={language}>{language}</option>}
    </SelectRow>
  );
}

/** 共享的语音启用、服务和语言控件：语音输入页的识别分组。 */
export function VoiceInputCoreSection({
  language,
  systemVoice,
  onLanguageChange,
  ...serviceRows
}: VoiceInputCoreSectionProps) {
  return (
    <GroupList title="识别">
      <VoiceInputServiceRows {...serviceRows} />
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
