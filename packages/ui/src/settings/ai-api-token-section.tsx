import { SecretInput } from "../core/secret-input";
import { Row } from "../core/platform-controls";
import { SecretSettingField } from "./secret-setting-field";

export interface AiApiTokenSectionProps {
  origin: string | null;
  token: string;
  onTokenChange: (token: string) => void;
  /** 画成 AI 辅助页「服务」组里的一行；不传时仍是旧面板的独立卡片。 */
  grouped?: boolean;
}

/** API token input for AI settings on hosts that keep credentials in the page. */
export function AiApiTokenSection({
  origin,
  token,
  onTokenChange,
  grouped = false,
}: AiApiTokenSectionProps) {
  if (grouped) {
    return (
      <Row
        title="API Token"
        description={origin ? `只用于 ${origin}` : "请先在「更多选项」中填写有效的 HTTPS 接口地址"}
      >
        <SecretInput
          label="AI API Token"
          disabled={!origin}
          value={token}
          onChange={onTokenChange}
        />
      </Row>
    );
  }
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
