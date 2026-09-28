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
      <label className="section-header">
        <span className="section-title">语音提示音</span>
        <input
          aria-label="语音提示音"
          className="toggle"
          type="checkbox"
          checked={soundEnabled}
          onChange={(event) => onSoundEnabledChange(event.target.checked)}
        />
      </label>
      <label className="section-header">
        <span className="section-title">开始录音提示音</span>
        <input
          aria-label="开始录音提示音"
          className="toggle"
          type="checkbox"
          checked={startSound}
          onChange={(event) => onStartSoundChange(event.target.checked)}
        />
      </label>
      <label className="section-header">
        <span className="section-title">结束录音提示音</span>
        <input
          aria-label="结束录音提示音"
          className="toggle"
          type="checkbox"
          checked={endSound}
          onChange={(event) => onEndSoundChange(event.target.checked)}
        />
      </label>
      <label className="section-header">
        <span className="section-title">录音时静音其他声音</span>
        <input
          aria-label="录音时静音其他声音"
          className="toggle"
          type="checkbox"
          checked={muteSystemAudio}
          onChange={(event) => onMuteSystemAudioChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
