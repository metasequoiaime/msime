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
  grouped?: boolean;
}

/** Shared recognition result controls; desktop pages group the rows while panels leave that layout to their host. */
export function VoiceRecognitionResultSection({
  showStreamPreedit,
  showCommitMode,
  macos,
  streamInlinePreedit,
  commitMode,
  onStreamInlinePreeditChange,
  onCommitModeChange,
  grouped = false,
}: VoiceRecognitionResultSectionProps) {
  if (!showStreamPreedit && !showCommitMode) return null;

  const content = (
    <>
      {showStreamPreedit && (
        <VoiceStreamPreeditSection
          enabled={streamInlinePreedit}
          onChange={onStreamInlinePreeditChange}
        />
      )}
      {showCommitMode && (
        <VoiceCommitModeSection macos={macos} value={commitMode} onChange={onCommitModeChange} />
      )}
    </>
  );

  return grouped ? <GroupList title="识别结果">{content}</GroupList> : content;
}
