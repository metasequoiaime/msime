import { Row, Switch } from "../core/platform-controls";

export interface CandidatePageIndicatorSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** The page number switch for hosts that can leave it out of the candidate window: one row of the 候选窗口 page's 翻页 group. */
export function CandidatePageIndicatorSection({ value, onChange }: CandidatePageIndicatorSectionProps) {
  return (
    <Row title="显示页码" description="在候选窗口显示当前页和总页数。关闭后翻页键照常可用。">
      <Switch checked={value ?? true} onChange={onChange} />
    </Row>
  );
}
