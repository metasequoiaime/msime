import { SettingsGroupNote } from "./settings-group-note";
import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";

export interface VoiceRecordingBehaviorSectionProps {
  linux: boolean;
  soundEnabled: boolean;
  muteSystemAudio: boolean;
  onSoundEnabledChange: (enabled: boolean) => void;
  onMuteSystemAudioChange: (enabled: boolean) => void;
}

/** Host-side recording prompts and audio muting preferences: the 录音行为 group on the voice page. 开始、结束提示音分开的两个开关在「服务配置」子页的 {@link VoiceCueSwitchesSection} 里。 */
export function VoiceRecordingBehaviorSection({
  linux,
  soundEnabled,
  muteSystemAudio,
  onSoundEnabledChange,
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
        title="录音时静音其他声音"
        aria-label="录音时静音其他声音"
        checked={muteSystemAudio}
        onChange={onMuteSystemAudioChange}
      />
    </GroupList>
  );
}

export interface VoiceCueSwitchesSectionProps {
  soundEnabled: boolean;
  startSound: boolean;
  endSound: boolean;
  onStartSoundChange: (enabled: boolean) => void;
  onEndSoundChange: (enabled: boolean) => void;
}

/** 开始、结束提示音各自的开关，只在分别读这两个值的宿主上给出（`voice_cue_switches`）。总开关关着时两个都不响，所以跟着变灰，与手机的做法一致。 */
export function VoiceCueSwitchesSection({
  soundEnabled,
  startSound,
  endSound,
  onStartSoundChange,
  onEndSoundChange,
}: VoiceCueSwitchesSectionProps) {
  return (
    <GroupList title="提示音">
      <SwitchRow
        title="开始录音提示音"
        aria-label="开始录音提示音"
        disabled={!soundEnabled}
        checked={startSound}
        onChange={onStartSoundChange}
      />
      <SwitchRow
        title="结束录音提示音"
        aria-label="结束录音提示音"
        disabled={!soundEnabled}
        checked={endSound}
        onChange={onEndSoundChange}
      />
    </GroupList>
  );
}
