import { SwitchRow } from "./switch-row";

export interface CandidateFollowCursorSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Candidate-window positioning switch for hosts that expose the setting: one row of the 候选窗口 page's 位置 group. */
export function CandidateFollowCursorSection({
  value,
  onChange,
}: CandidateFollowCursorSectionProps) {
  return (
    <SwitchRow
      title="候选窗口跟随光标"
      description="关闭后保持首次出现的位置，直到候选窗口消失。"
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
