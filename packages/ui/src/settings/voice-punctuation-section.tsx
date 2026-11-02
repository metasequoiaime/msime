import { SelectRow } from "./select-row";

export type VoicePunctuationMode = "auto" | "omit_trailing" | "omit_all" | "space";

export interface VoicePunctuationSectionProps {
  value: VoicePunctuationMode;
  onChange: (value: VoicePunctuationMode) => void;
}

/** 语音识别结果上屏前的标点处理方式。 */
export function VoicePunctuationSection({ value, onChange }: VoicePunctuationSectionProps) {
  return (
    <SelectRow
      title="语音标点"
      description="对所有识别服务和 AI 润色后的结果生效，下一次识别起作用"
      aria-label="语音标点"
      value={value}
      onChange={(event) => onChange(event.target.value as VoicePunctuationMode)}
    >
      <option value="auto">智能添加</option>
      <option value="omit_trailing">省略句末标点</option>
      <option value="omit_all">省略所有标点</option>
      <option value="space">空格代替标点</option>
    </SelectRow>
  );
}
