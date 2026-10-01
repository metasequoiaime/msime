import type { ReactNode } from "react";
import { Row, Switch } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SecretSettingRow } from "./secret-setting-row";

export interface TencentTranslationSectionProps {
  enabled: boolean;
  available: boolean;
  secretId: string;
  secretKey: string;
  region: string;
  credentialIssue?: string;
  showMissingCredentialsWarning?: boolean;
  onToggle: (enabled: boolean) => void;
  onSecretIdChange: (value: string) => void;
  onSecretKeyChange: (value: string) => void;
  onRegionChange: (value: string) => void;
  children?: ReactNode;
}

/** Tencent Cloud candidate translation settings and credentials. */
export function TencentTranslationSection({
  enabled,
  available,
  secretId,
  secretKey,
  region,
  credentialIssue,
  showMissingCredentialsWarning = false,
  onToggle,
  onSecretIdChange,
  onSecretKeyChange,
  onRegionChange,
  children,
}: TencentTranslationSectionProps) {
  return (
    <div role="group" aria-label="在线翻译服务" className={settings.rowStack}>
      <Row
        title="在线翻译服务"
        description="候选词翻译默认使用腾讯云机器翻译，需要填入你自己的 API 凭据"
      >
        <Switch
          aria-label="腾讯云机器翻译"
          disabled={!available}
          checked={enabled}
          onChange={onToggle}
        />
      </Row>
      <Row title="SecretId">
        <input
          aria-label="腾讯云 SecretId"
          type="text"
          autoComplete="off"
          spellCheck={false}
          value={secretId}
          disabled={!available || !enabled}
          onChange={(event) => onSecretIdChange(event.target.value)}
          placeholder="AKIDxxxxxxxxxxxxxxxx"
        />
      </Row>
      <SecretSettingRow
        title="SecretKey"
        label="腾讯云 SecretKey"
        value={secretKey}
        disabled={!available || !enabled}
        onChange={onSecretKeyChange}
      />
      <Row title="地域">
        <input
          aria-label="腾讯云地域"
          type="text"
          autoComplete="off"
          spellCheck={false}
          value={region}
          disabled={!available || !enabled}
          onChange={(event) => onRegionChange(event.target.value)}
          placeholder="ap-guangzhou"
        />
      </Row>
      {enabled && children && <div className={settings.groupBlock}>{children}</div>}
      {available && enabled && credentialIssue && (
        <div className={settings.groupBlock}>
          <p className={settings.settingsWarning} role="status">
            {credentialIssue}
          </p>
        </div>
      )}
      {available && enabled && !credentialIssue && showMissingCredentialsWarning && (
        <div className={settings.groupBlock}>
          <p className={settings.settingsWarning} role="status">
            未填写腾讯云凭据，候选词翻译不会有任何结果。请填入 SecretId 与
            SecretKey，或改用下面的自定义翻译服务。
          </p>
        </div>
      )}
    </div>
  );
}
