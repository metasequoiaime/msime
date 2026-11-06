import { GroupList } from "../core/platform-controls";
import { VoiceCommitModeSection, type VoiceCommitMode } from "./voice-commit-mode-section";
import { VoiceStreamPreeditSection } from "./voice-stream-preedit-section";
import { VoicePunctuationSection, type VoicePunctuationMode } from "./voice-punctuation-section";

export interface VoiceRecognitionResultSectionProps {
  showStreamPreedit: boolean;
  showCommitMode: boolean;
  showPunctuation: boolean;
  macos: boolean;
  streamInlinePreedit: boolean;
  commitMode: VoiceCommitMode;
  punctuationMode: VoicePunctuationMode;
  onStreamInlinePreeditChange: (enabled: boolean) => void;
  onCommitModeChange: (mode: VoiceCommitMode) => void;
  onPunctuationModeChange: (mode: VoicePunctuationMode) => void;
}

/** Shared recognition result controls: the 识别结果 group. */
export function VoiceRecognitionResultSection({
  showStreamPreedit,
  showCommitMode,
  showPunctuation,
  macos,
  streamInlinePreedit,
  commitMode,
  punctuationMode,
  onStreamInlinePreeditChange,
  onCommitModeChange,
  onPunctuationModeChange,
}: VoiceRecognitionResultSectionProps) {
  if (!showStreamPreedit && !showCommitMode && !showPunctuation) return null;

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
      {showPunctuation && (
        <VoicePunctuationSection value={punctuationMode} onChange={onPunctuationModeChange} />
      )}
    </GroupList>
  );
}
