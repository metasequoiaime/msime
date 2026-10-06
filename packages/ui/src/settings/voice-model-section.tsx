import { TextInputRow } from "./text-input-row";

export interface VoiceModelSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Provider-selected speech recognition model field, a row of the 识别服务配置 group. */
export function VoiceModelSection({ value, onChange }: VoiceModelSectionProps) {
  return (
    <TextInputRow
      title="识别模型"
      description="由语音服务选择对应模型"
      label="识别模型"
      value={value}
      onChange={onChange}
    />
  );
}
