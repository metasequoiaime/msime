import { Row } from "../core/platform-controls";

export interface VoiceEndpointSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Endpoint override for a remote voice recognition provider. */
export function VoiceEndpointSection({ value, onChange }: VoiceEndpointSectionProps) {
  return (
    <Row title="识别接口地址" description="留空使用当前 provider 默认地址">
      <input
        aria-label="识别接口地址"
        type="url"
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </Row>
  );
}
