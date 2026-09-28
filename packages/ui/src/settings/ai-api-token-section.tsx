import { SecretInput } from "../core/secret-input";

export interface AiApiTokenSectionProps {
  origin: string | null;
  token: string;
  onTokenChange: (token: string) => void;
}

/** API token input for AI settings on hosts that keep credentials in the page. */
export function AiApiTokenSection({ origin, token, onTokenChange }: AiApiTokenSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          API Token
          <small>{origin ? `只用于 ${origin}` : "请先填写有效的 HTTPS 接口地址"}</small>
        </span>
        <SecretInput
          label="AI API Token"
          disabled={!origin}
          value={token}
          onChange={onTokenChange}
        />
      </label>
    </div>
  );
}
