import { SwitchRow } from "./switch-row";

export interface CandidateFollowCursorSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 候选窗口定位开关，供提供该设置的宿主使用：「候选窗口」页「布局」组中的一行。 */
export function CandidateFollowCursorSection({
  value,
  onChange,
}: CandidateFollowCursorSectionProps) {
  return (
    <SwitchRow
      title="跟随光标"
      description="关闭后保持首次出现的位置，直到候选窗口消失。"
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
