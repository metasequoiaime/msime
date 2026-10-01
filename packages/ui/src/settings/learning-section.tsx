import { SwitchRow } from "./switch-row";

export interface LearningSectionProps {
  value: boolean;
  onChange: (value: boolean) => void;
}

/** 有输入设置页的宿主共用的学习开关：输入页「候选与联想」组里的一行。 */
export function LearningSection({ value, onChange }: LearningSectionProps) {
  return (
    <SwitchRow
      title="学习选词习惯"
      description="根据选词调整候选顺序"
      checked={value}
      onChange={onChange}
    />
  );
}
