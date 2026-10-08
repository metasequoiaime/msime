import { SecretSettingRow } from "./secret-setting-row";

export interface AiApiTokenSectionProps {
  origin: string | null;
  /** 接口地址不能用时的说明，见 `aiEndpointHint`。 */
  endpointHint?: string;
  token: string;
  onTokenChange: (token: string) => void;
}

/** API token input for AI settings on hosts that keep credentials in the page. */
export function AiApiTokenSection({
  origin,
  endpointHint,
  token,
  onTokenChange,
}: AiApiTokenSectionProps) {
  return (
    <SecretSettingRow
      title="API Token"
      description={
        origin ? `只用于 ${origin}` : endpointHint || "请先在「更多选项」中填写有效的接口地址"
      }
      label="AI API Token"
      disabled={!origin}
      value={token}
      onChange={onTokenChange}
    />
  );
}
