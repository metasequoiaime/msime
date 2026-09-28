import { SecretInput } from "../core/secret-input";

export interface VoiceCredentialFieldsSectionProps {
  showAppKey: boolean;
  appKey: string;
  tokenLabel: string;
  token: string;
  onAppKeyChange: (value: string) => void;
  onTokenChange: (value: string) => void;
}

/** Shared App Key and API token fields for remote voice providers. */
export function VoiceCredentialFieldsSection({
  showAppKey,
  appKey,
  tokenLabel,
  token,
  onAppKeyChange,
  onTokenChange,
}: VoiceCredentialFieldsSectionProps) {
  return (
    <>
      {showAppKey && (
        <div className="section">
          <label className="section-header">
            <span className="section-title">
              Doubao App Key<small>旧版控制台鉴权使用</small>
            </span>
            <SecretInput label="Doubao App Key" value={appKey} onChange={onAppKeyChange} />
          </label>
        </div>
      )}
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            {tokenLabel}
            <small>仅保存在本机设置中</small>
          </span>
          <SecretInput label={tokenLabel} value={token} onChange={onTokenChange} />
        </label>
      </div>
    </>
  );
}
