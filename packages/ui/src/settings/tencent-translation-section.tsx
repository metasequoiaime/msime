import type { ReactNode } from "react";
import * as settings from "./settings-style";
import { SecretSettingRow } from "./secret-setting-row";
import { TextInputRow } from "./text-input-row";

export interface TencentTranslationSectionProps {
  available: boolean;
  secretId: string;
  secretKey: string;
  region: string;
  credentialIssue?: string;
  showMissingCredentialsWarning?: boolean;
  onSecretIdChange: (value: string) => void;
  onSecretKeyChange: (value: string) => void;
  onRegionChange: (value: string) => void;
  children?: ReactNode;
}

/** Tencent Cloud credentials, shown while Tencent is the chosen translation service; the service itself is chosen in 翻译服务. */
export function TencentTranslationSection({
  available,
  secretId,
  secretKey,
  region,
  credentialIssue,
  showMissingCredentialsWarning = false,
  onSecretIdChange,
  onSecretKeyChange,
  onRegionChange,
  children,
}: TencentTranslationSectionProps) {
  return (
    <div role="group" aria-label="腾讯云机器翻译" className={settings.rowStack}>
      <p className={settings.groupNote}>需要填入你自己的腾讯云 API 凭据。</p>
      <TextInputRow
        title="SecretId"
        label="腾讯云 SecretId"
        type="text"
        autoComplete="off"
        spellCheck={false}
        value={secretId}
        disabled={!available}
        onChange={onSecretIdChange}
        placeholder="AKIDxxxxxxxxxxxxxxxx"
      />
      <SecretSettingRow
        title="SecretKey"
        label="腾讯云 SecretKey"
        value={secretKey}
        disabled={!available}
        onChange={onSecretKeyChange}
      />
      <TextInputRow
        title="地域"
        label="腾讯云地域"
        type="text"
        autoComplete="off"
        spellCheck={false}
        value={region}
        disabled={!available}
        onChange={onRegionChange}
        placeholder="ap-guangzhou"
      />
      {children && <div className={settings.groupBlock}>{children}</div>}
      {available && credentialIssue && (
        <div className={settings.groupBlock}>
          <p className={settings.settingsWarning} role="status">
            {credentialIssue}
          </p>
        </div>
      )}
      {available && !credentialIssue && showMissingCredentialsWarning && (
        <div className={settings.groupBlock}>
          <p className={settings.settingsWarning} role="status">
            未填写腾讯云凭据，候选词翻译不会有任何结果。请填入 SecretId 与
            SecretKey，或在上面的翻译服务中改选其他服务。
          </p>
        </div>
      )}
    </div>
  );
}
