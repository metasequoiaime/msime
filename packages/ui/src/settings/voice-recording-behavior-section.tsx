export interface VoiceRecordingBehaviorSectionProps {
  linux: boolean;
  soundEnabled: boolean;
  startSound: boolean;
  endSound: boolean;
  muteSystemAudio: boolean;
  onSoundEnabledChange: (enabled: boolean) => void;
  onStartSoundChange: (enabled: boolean) => void;
  onEndSoundChange: (enabled: boolean) => void;
  onMuteSystemAudioChange: (enabled: boolean) => void;
}

/** Host-side recording prompts and audio muting preferences. */
export function VoiceRecordingBehaviorSection({
  linux,
  soundEnabled,
  startSound,
  endSound,
  muteSystemAudio,
  onSoundEnabledChange,
  onStartSoundChange,
  onEndSoundChange,
  onMuteSystemAudioChange,
}: VoiceRecordingBehaviorSectionProps) {
  return (
    <div className="section">
      <div className="section-title">
        {linux ? "Linux provider 行为" : "录音行为"}
        <small>
          {linux
            ? "这些选项会随请求传给用户管理的语音服务，不包含凭据"
            : "录音期间的提示音与静音由输入法在本机处理"}
        </small>
      </div>
      <SettingToggle
        label="语音提示音"
        ariaLabel="语音提示音"
        checked={soundEnabled}
        compact
        onChange={onSoundEnabledChange}
      />
      <SettingToggle
        label="开始录音提示音"
        ariaLabel="开始录音提示音"
        checked={startSound}
        compact
        onChange={onStartSoundChange}
      />
      <SettingToggle
        label="结束录音提示音"
        ariaLabel="结束录音提示音"
        checked={endSound}
        compact
        onChange={onEndSoundChange}
      />
      <SettingToggle
        label="录音时静音其他声音"
        ariaLabel="录音时静音其他声音"
        checked={muteSystemAudio}
        compact
        onChange={onMuteSystemAudioChange}
      />
    </div>
  );
}
import { SettingToggle } from "./setting-toggle";
