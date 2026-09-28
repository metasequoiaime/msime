import { SecretInput } from "../core/secret-input";

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
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            润色接口地址<small>留空使用当前 provider 默认地址</small>
          </span>
          <input
            aria-label="润色接口地址"
            type="url"
            value={endpoint}
            onChange={(event) => onEndpointChange(event.target.value)}
          />
        </label>
      </div>
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            润色 API Token<small>仅保存在本机设置中</small>
          </span>
          <SecretInput label="润色 API Token" value={token} onChange={onTokenChange} />
        </label>
      </div>
    </>
  );
}
