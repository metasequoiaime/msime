import { SelectRow } from "./select-row";
import type { VoiceInputPreferences } from "./voice-input-preferences";

export type VoicePunctuationMode = NonNullable<VoiceInputPreferences["punctuation_mode"]>;

export const voicePunctuationOptions: readonly { value: VoicePunctuationMode; label: string }[] = [
  { value: "auto", label: "智能添加" },
  { value: "omit_trailing", label: "省略句末标点" },
  { value: "omit_all", label: "省略所有标点" },
  { value: "space", label: "空格代替标点" },
];

export interface VoicePunctuationSectionProps {
  value: VoicePunctuationMode;
  onChange: (value: VoicePunctuationMode) => void;
}

/** 识别结果上屏前的标点处理，所有识别服务和语音润色结果共用。 */
export function VoicePunctuationSection({ value, onChange }: VoicePunctuationSectionProps) {
  return (
    <SelectRow
      title="语音标点"
      description="对所有识别服务和 AI 润色后的结果生效，下一次识别起作用"
      aria-label="语音标点"
      value={value}
      onChange={(event) => onChange(event.target.value as VoicePunctuationMode)}
    >
      {voicePunctuationOptions.map(({ value: option, label }) => (
        <option key={option} value={option}>
          {label}
        </option>
      ))}
    </SelectRow>
  );
}
