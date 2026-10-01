import { SecretSettingField } from "./secret-setting-field";

export interface AiApiTokenSectionProps {
  origin: string | null;
  token: string;
  onTokenChange: (token: string) => void;
}

/** API token input for AI settings on hosts that keep credentials in the page. */
export function AiApiTokenSection({ origin, token, onTokenChange }: AiApiTokenSectionProps) {
  return (
    <div className="section">
      <SecretSettingField
        label="API Token"
        inputLabel="AI API Token"
        value={token}
        description={origin ? `只用于 ${origin}` : "请先填写有效的 HTTPS 接口地址"}
        disabled={!origin}
        onChange={onTokenChange}
      />
    </div>
  );
}
