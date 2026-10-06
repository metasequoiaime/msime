import { SettingsGroupNote } from "./settings-group-note";
import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";

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
    <GroupList title="录音行为">
      <SettingsGroupNote>
        {linux
          ? "这些选项会随请求传给用户管理的语音服务，不包含凭据"
          : "录音期间的提示音与静音由输入法在本机处理"}
      </SettingsGroupNote>
      <SwitchRow
        title="语音提示音"
        aria-label="语音提示音"
        checked={soundEnabled}
        onChange={onSoundEnabledChange}
      />
      <SwitchRow
        title="开始录音提示音"
        aria-label="开始录音提示音"
        checked={startSound}
        onChange={onStartSoundChange}
      />
      <SwitchRow
        title="结束录音提示音"
        aria-label="结束录音提示音"
        checked={endSound}
        onChange={onEndSoundChange}
      />
      <SwitchRow
        title="录音时静音其他声音"
        aria-label="录音时静音其他声音"
        checked={muteSystemAudio}
        onChange={onMuteSystemAudioChange}
      />
    </GroupList>
  );
}
