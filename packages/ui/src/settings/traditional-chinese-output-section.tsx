import { SettingToggle } from "./setting-toggle";

export interface TraditionalChineseOutputSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Simplified-to-traditional output switch shared by input settings hosts. */
export function TraditionalChineseOutputSection({
  value,
  onChange,
}: TraditionalChineseOutputSectionProps) {
  return (
    <SettingToggle
      label="简繁输入"
      description="将提交的简体中文转换为繁体中文"
      ariaLabel="简繁输入"
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
