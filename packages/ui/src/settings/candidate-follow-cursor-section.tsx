import { SettingToggle } from "./setting-toggle";

export interface CandidateFollowCursorSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Candidate-window positioning switch for hosts that expose the setting. */
export function CandidateFollowCursorSection({
  value,
  onChange,
}: CandidateFollowCursorSectionProps) {
  return (
    <SettingToggle
      label="候选窗口跟随光标"
      description="关闭后保持首次出现的位置，直到候选窗口消失。"
      ariaLabel="候选窗口跟随光标"
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
