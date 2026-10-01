import { EndpointInput } from "./endpoint-input";
import { SecretSettingField } from "./secret-setting-field";
import { SettingField } from "./setting-field";

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
      <SettingField label="润色接口地址" description="留空使用当前 provider 默认地址">
        <EndpointInput
          label="润色接口地址"
          value={endpoint}
          onChange={onEndpointChange}
        />
      </SettingField>
      <SecretSettingField
        label="润色 API Token"
        description="仅保存在本机设置中"
        value={token}
        onChange={onTokenChange}
      />
    </>
  );
}
