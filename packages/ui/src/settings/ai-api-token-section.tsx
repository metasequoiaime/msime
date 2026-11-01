import { SecretSettingRow } from "./secret-setting-row";
import { aiEndpointServiceAddress } from "./ai-endpoint-policy";

export interface AiApiTokenSectionProps {
  origin: string | null;
  /** 展示接口的服务地址；Token 仍按 origin 保存。 */
  endpoint?: string;
  /** 接口地址不能用时的说明，见 `aiEndpointHint`。 */
  endpointHint?: string;
  token: string;
  onTokenChange: (token: string) => void;
}

/** 在页面中保存凭据的宿主使用的 AI Token 输入行。 */
export function AiApiTokenSection({
  origin,
  endpoint,
  endpointHint,
  token,
  onTokenChange,
}: AiApiTokenSectionProps) {
  return (
    <SecretSettingRow
      title="API Token"
      detail={
        origin
          ? `服务地址：${aiEndpointServiceAddress(endpoint ?? origin) ?? origin}`
          : endpointHint || "请先填写有效的接口地址"
      }
      label="AI API Token"
      disabled={!origin}
      value={token}
      onChange={onTokenChange}
    />
  );
}
