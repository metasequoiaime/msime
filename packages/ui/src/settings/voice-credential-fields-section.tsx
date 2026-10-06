import { SecretSettingField } from "./secret-setting-field";

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
        <SecretSettingField
          label="Doubao App Key"
          description="旧版控制台鉴权使用"
          value={appKey}
          onChange={onAppKeyChange}
        />
      )}
      <SecretSettingField
        label={tokenLabel}
        description="仅保存在本机设置中"
        value={token}
        onChange={onTokenChange}
      />
    </>
  );
}
