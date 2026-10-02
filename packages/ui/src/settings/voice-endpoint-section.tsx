import { EndpointSettingRow } from "./endpoint-setting-row";

export interface VoiceEndpointSectionProps {
  value: string;
  onChange: (value: string) => void;
}

/** Endpoint override for a remote voice recognition provider. */
export function VoiceEndpointSection({ value, onChange }: VoiceEndpointSectionProps) {
  return (
    <EndpointSettingRow
      title="识别接口地址"
      description="留空使用当前服务的默认地址"
      inputLabel="识别接口地址"
      value={value}
      onChange={onChange}
    />
  );
}
