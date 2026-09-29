import { Row, Select } from "../core/platform-controls";

export type DefaultImeMode = "chinese" | "english";

export interface DefaultImeModeSectionProps {
  value?: DefaultImeMode;
  onChange: (value: DefaultImeMode) => void;
}

/** Default language mode selector for new input focus sessions: one row of the 中英文 group. */
export function DefaultImeModeSection({ value, onChange }: DefaultImeModeSectionProps) {
  return (
    <Row title="默认中英文" description="新焦点会话开始时使用的中文或英文状态">
      <Select
        value={value ?? "chinese"}
        onChange={(event) => onChange(event.target.value as DefaultImeMode)}
      >
        <option value="chinese">中文</option>
        <option value="english">英文</option>
      </Select>
    </Row>
  );
}
