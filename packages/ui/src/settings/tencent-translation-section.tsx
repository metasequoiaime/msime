import { SettingsGroupNote } from "./settings-group-note";
import { SettingsWarning } from "./settings-warning";
import type { ReactNode } from "react";
import * as settings from "./settings-style";
import { SecretSettingRow } from "./secret-setting-row";
import { TextInputRow } from "./text-input-row";
import { SettingsGroupBlock } from "./settings-group-block";

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

/** 腾讯云凭据，在腾讯翻译是所选翻译服务时显示；服务本身在「翻译服务」中选择。 */
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
      <SettingsGroupNote>需要填入你自己的腾讯云 API 凭据。</SettingsGroupNote>
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
      {children && <SettingsGroupBlock>{children}</SettingsGroupBlock>}
      {available && credentialIssue && (
        <SettingsGroupBlock>
          <SettingsWarning>{credentialIssue}</SettingsWarning>
        </SettingsGroupBlock>
      )}
      {available && !credentialIssue && showMissingCredentialsWarning && (
        <SettingsGroupBlock>
          <SettingsWarning>
            未填写腾讯云凭据，候选词翻译不会有任何结果。请填入 SecretId 与
            SecretKey，或在上面的翻译服务中改选其他服务。
          </SettingsWarning>
        </SettingsGroupBlock>
      )}
    </div>
  );
}
