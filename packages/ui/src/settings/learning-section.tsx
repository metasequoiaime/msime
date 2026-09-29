import { Row, Switch } from "../core/platform-controls";

export interface LearningSectionProps {
  value: boolean;
  onChange: (value: boolean) => void;
}

/** Learning preference switch shared by hosts that expose the input settings page: one row of the 选词 group. */
export function LearningSection({ value, onChange }: LearningSectionProps) {
  return (
    <Row title="学习选词习惯" description="根据选词调整候选顺序">
      <Switch checked={value} onChange={onChange} />
    </Row>
  );
}
