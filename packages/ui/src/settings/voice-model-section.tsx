import { Row } from "../core/platform-controls";

export interface VoiceModelSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Provider-selected speech recognition model field, a row of the 识别服务配置 group. */
export function VoiceModelSection({ value, onChange }: VoiceModelSectionProps) {
  return (
    <Row title="识别模型" description="由 provider 服务选择对应模型">
      <input
        aria-label="识别模型"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </Row>
  );
}
