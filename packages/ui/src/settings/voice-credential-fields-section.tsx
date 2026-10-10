import { SecretSettingRow } from "./secret-setting-row";

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
        <SecretSettingRow
          title="Doubao App Key"
          description="旧版控制台鉴权使用"
          label="Doubao App Key"
          value={appKey}
          onChange={onAppKeyChange}
        />
      )}
      <SecretSettingRow
        title={tokenLabel}
        description="仅保存在本机设置中"
        label={tokenLabel}
        value={token}
        onChange={onTokenChange}
      />
    </>
  );
}
