import { Row } from "../core/platform-controls";
import { EndpointInput } from "./endpoint-input";

export interface VoiceEndpointSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Endpoint override for a remote voice recognition provider. */
export function VoiceEndpointSection({ value, onChange }: VoiceEndpointSectionProps) {
  return (
    <Row title="识别接口地址" description="留空使用当前 provider 默认地址">
      <EndpointInput
        label="识别接口地址"
        value={value}
        onChange={onChange}
      />
    </Row>
  );
}
