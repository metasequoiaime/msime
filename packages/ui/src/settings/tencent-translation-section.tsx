import type { ReactNode } from "react";
import { SecretInput } from "../core/secret-input";
import * as settings from "./settings-style";

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
    <div className="section" role="group" aria-label="在线翻译服务">
      <label className="section-header">
        <span className="section-title">
          在线翻译服务
          <small>候选词翻译默认使用腾讯云机器翻译，需要填入你自己的 API 凭据</small>
        </span>
        <input
          aria-label="腾讯云机器翻译"
          className="toggle"
          type="checkbox"
          disabled={!available}
          checked={enabled}
          onChange={(event) => onToggle(event.target.checked)}
        />
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">SecretId</span>
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
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">SecretKey</span>
        <SecretInput
          label="腾讯云 SecretKey"
          value={secretKey}
          disabled={!available || !enabled}
          onChange={onSecretKeyChange}
        />
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">地域</span>
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
      </label>
      {enabled && children}
      {available && enabled && credentialIssue && (
        <p className={settings.settingsWarning} role="status">
          {credentialIssue}
        </p>
      )}
      {available && enabled && !credentialIssue && showMissingCredentialsWarning && (
        <p className={settings.settingsWarning} role="status">
          未填写腾讯云凭据，候选词翻译不会有任何结果。请填入 SecretId 与
          SecretKey，或改用下面的自定义翻译服务。
        </p>
      )}
    </div>
  );
}
