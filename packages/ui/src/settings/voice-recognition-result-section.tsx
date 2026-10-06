import { GroupList } from "../core/platform-controls";
import { VoiceCommitModeSection, type VoiceCommitMode } from "./voice-commit-mode-section";
import { VoiceStreamPreeditSection } from "./voice-stream-preedit-section";

export interface VoiceRecognitionResultSectionProps {
  showStreamPreedit: boolean;
  showCommitMode: boolean;
  macos: boolean;
  streamInlinePreedit: boolean;
  commitMode: VoiceCommitMode;
  onStreamInlinePreeditChange: (enabled: boolean) => void;
  onCommitModeChange: (mode: VoiceCommitMode) => void;
}

/** Shared recognition result controls: the 识别结果 group. */
export function VoiceRecognitionResultSection({
  showStreamPreedit,
  showCommitMode,
  macos,
  streamInlinePreedit,
  commitMode,
  onStreamInlinePreeditChange,
  onCommitModeChange,
}: VoiceRecognitionResultSectionProps) {
  if (!showStreamPreedit && !showCommitMode) return null;

  return (
    <GroupList title="识别结果">
      {showStreamPreedit && (
        <VoiceStreamPreeditSection
          enabled={streamInlinePreedit}
          onChange={onStreamInlinePreeditChange}
        />
      )}
      {showCommitMode && (
        <VoiceCommitModeSection macos={macos} value={commitMode} onChange={onCommitModeChange} />
      )}
    </GroupList>
  );
}
