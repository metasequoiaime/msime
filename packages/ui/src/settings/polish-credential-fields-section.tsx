import { SecretInput } from "../core/secret-input";
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
        <input
          aria-label="润色接口地址"
          type="url"
          value={endpoint}
          onChange={(event) => onEndpointChange(event.target.value)}
        />
      </SettingField>
      <SettingField label="润色 API Token" description="仅保存在本机设置中">
        <SecretInput label="润色 API Token" value={token} onChange={onTokenChange} />
      </SettingField>
    </>
  );
}
