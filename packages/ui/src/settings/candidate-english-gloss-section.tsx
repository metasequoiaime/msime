import { Row, Switch } from "../core/platform-controls";

export interface CandidateEnglishGlossSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Offline English gloss switch for hosts that expose candidate annotations: one row of the 多语言候选 group on the 表达 page. */
export function CandidateEnglishGlossSection({
  value,
  onChange,
}: CandidateEnglishGlossSectionProps) {
  return (
    <Row
      title="显示英文释义"
      description="在候选词后面标出它的英文意思，中文候选给英文、英文候选给中文。释义来自随键盘打包的离线词库，不联网。"
    >
      <Switch checked={value ?? false} onChange={onChange} />
    </Row>
  );
}
