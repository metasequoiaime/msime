import { SecretInput } from "../core/secret-input";
import { SettingField } from "./setting-field";

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
        <SettingField label="Doubao App Key" description="旧版控制台鉴权使用">
          <SecretInput label="Doubao App Key" value={appKey} onChange={onAppKeyChange} />
        </SettingField>
      )}
      <SettingField label={tokenLabel} description="仅保存在本机设置中">
        <SecretInput label={tokenLabel} value={token} onChange={onTokenChange} />
      </SettingField>
    </>
  );
}
