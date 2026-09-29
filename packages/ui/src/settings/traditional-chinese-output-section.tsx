import { Row, Switch } from "../core/platform-controls";

export interface TraditionalChineseOutputSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Simplified-to-traditional output switch shared by input settings hosts: one row of the 输出 group. */
export function TraditionalChineseOutputSection({
  value,
  onChange,
}: TraditionalChineseOutputSectionProps) {
  return (
    <Row title="简繁输入" description="将提交的简体中文转换为繁体中文">
      <Switch checked={value ?? false} onChange={onChange} />
    </Row>
  );
}
