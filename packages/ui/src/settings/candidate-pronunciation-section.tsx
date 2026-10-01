import { Row, Switch } from "../core/platform-controls";

export interface CandidatePronunciationSectionProps {
  value?: boolean;
  disabled?: boolean;
  onChange: (value: boolean) => void;
}

/** Reading switch for hosts that draw one after each gloss line: one row of the 多语言候选 group on the 表达 page. It is disabled while every gloss source is off, since there is then nothing to read, but keeps its value. */
export function CandidatePronunciationSection({
  value,
  disabled,
  onChange,
}: CandidatePronunciationSectionProps) {
  return (
    <Row
      title="显示读音"
      description="在释义后面标出怎么读：英文释义给音标，日文释义给罗马音。音标来自随输入法打包的离线词表，罗马音由系统生成，都不联网。需要先打开释义。"
    >
      <Switch checked={value ?? false} disabled={disabled} onChange={onChange} />
    </Row>
  );
}
