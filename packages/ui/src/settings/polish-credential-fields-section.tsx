import { EndpointSettingRow } from "./endpoint-setting-row";
import { SecretSettingRow } from "./secret-setting-row";

export interface PolishCredentialFieldsSectionProps {
  endpoint: string;
  token: string;
  onEndpointChange: (value: string) => void;
  onTokenChange: (value: string) => void;
}

/** 文本润色服务的接口地址和 API Token，是「文本润色」分组里的两行。 */
export function PolishCredentialFieldsSection({
  endpoint,
  token,
  onEndpointChange,
  onTokenChange,
}: PolishCredentialFieldsSectionProps) {
  return (
    <>
      <EndpointSettingRow
        title="润色接口地址"
        inputLabel="润色接口地址"
        description="留空使用当前服务的默认地址"
        value={endpoint}
        onChange={onEndpointChange}
      />
      <SecretSettingRow
        title="润色 API Token"
        label="润色 API Token"
        description="仅保存在本机设置中"
        value={token}
        onChange={onTokenChange}
      />
    </>
  );
}
