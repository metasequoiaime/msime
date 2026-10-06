import { SecretSettingRow } from "./secret-setting-row";

export interface AiApiTokenSectionProps {
  origin: string | null;
  token: string;
  onTokenChange: (token: string) => void;
}

/** API token input for AI settings on hosts that keep credentials in the page. */
export function AiApiTokenSection({ origin, token, onTokenChange }: AiApiTokenSectionProps) {
  return (
    <SecretSettingRow
      title="API Token"
      description={origin ? `只用于 ${origin}` : "请先在「更多选项」中填写有效的 HTTPS 接口地址"}
      label="AI API Token"
      disabled={!origin}
      value={token}
      onChange={onTokenChange}
    />
  );
}
