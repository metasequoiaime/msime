import { EndpointSettingField } from "./endpoint-setting-field";
import { SecretSettingField } from "./secret-setting-field";

export interface PolishCredentialFieldsSectionProps {
  endpoint: string;
  token: string;
  onEndpointChange: (value: string) => void;
  onTokenChange: (value: string) => void;
}

/** Endpoint override and API token fields for the text polishing provider. */
export function PolishCredentialFieldsSection({
  endpoint,
  token,
  onEndpointChange,
  onTokenChange,
}: PolishCredentialFieldsSectionProps) {
  return (
    <>
      <EndpointSettingField
        label="润色接口地址"
        inputLabel="润色接口地址"
        description="留空使用当前服务的默认地址"
        value={endpoint}
        onChange={onEndpointChange}
      />
      <SecretSettingField
        label="润色 API Token"
        description="仅保存在本机设置中"
        value={token}
        onChange={onTokenChange}
      />
    </>
  );
}
