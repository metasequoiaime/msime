import { SettingToggle } from "./setting-toggle";

export interface LearningSectionProps {
  value: boolean;
  onChange: (value: boolean) => void;
}

/** Learning preference switch shared by hosts that expose the input settings page. */
export function LearningSection({ value, onChange }: LearningSectionProps) {
  return (
    <SettingToggle
      label="学习选词习惯"
      description="根据选词调整候选顺序"
      ariaLabel="学习选词习惯"
      checked={value}
      onChange={onChange}
    />
  );
}
