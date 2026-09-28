import {
  CredentialStatusMessage,
  type CredentialStatusMessageValue,
} from "./credential-status-message";
import type { ReactNode } from "react";

export interface LinuxTencentCredentialStatus {
  tencent: { region: string } | null;
  tencentInvalid: boolean;
}

export interface LinuxTencentCredentialInput {
  secretId: string;
  secretKey: string;
  region?: string;
}

export interface LinuxTencentCredentialsSectionProps {
  available: boolean;
  status?: LinuxTencentCredentialStatus;
  input: LinuxTencentCredentialInput;
  busy: boolean;
  message?: CredentialStatusMessageValue;
  onInputChange: (patch: Partial<LinuxTencentCredentialInput>) => void;
  onSave: (credential: { secretId?: string; secretKey?: string; region: string }) => void;
  onClear: () => void;
  children?: ReactNode;
}

/** Linux provider credentials for Tencent translation, without exposing stored secrets. */
export function LinuxTencentCredentialsSection({
  status,
  available,
  input,
  busy,
  message,
  onInputChange,
  onSave,
  onClear,
  children,
}: LinuxTencentCredentialsSectionProps) {
  const region = input.region ?? status?.tencent?.region ?? "ap-guangzhou";
  const stored = status?.tencent;
  const description = status?.tencentInvalid
    ? "现有 tencent-provider.json 无效，provider 服务不会发出翻译请求；请修复或删除该文件。"
    : stored
      ? "腾讯云凭据已保存；SecretId 和 SecretKey 留空则保留原值。"
      : "凭据只写入用户配置目录的 tencent-provider.json，由 provider 服务读取，不进入共享设置。";

  return (
    <div className="section" role="group" aria-label="在线翻译服务">
      <div className="section-title">
        在线翻译服务
        <small>由用户管理的 Linux provider 服务负责网络请求和凭据</small>
      </div>
      {!available ? (
        <p className="input-setting-description">
          候选词翻译开启后，provider 从用户配置目录的 <code>tencent-provider.json</code>{" "}
          读取腾讯云凭据；设置页不保存不会生效的 SecretId 或 SecretKey。
        </p>
      ) : (
        <>
          <p className="input-setting-description">{description}</p>
          <label className="section-header">
            <span className="section-title">SecretId</span>
            <input
              aria-label="腾讯云 SecretId"
              type="password"
              autoComplete="off"
              value={input.secretId}
              onChange={(event) => onInputChange({ secretId: event.target.value })}
            />
          </label>
          <label className="section-header">
            <span className="section-title">SecretKey</span>
            <input
              aria-label="腾讯云 SecretKey"
              type="password"
              autoComplete="off"
              value={input.secretKey}
              onChange={(event) => onInputChange({ secretKey: event.target.value })}
            />
          </label>
          <label className="section-header">
            <span className="section-title">地域</span>
            <input
              aria-label="腾讯云地域"
              value={region}
              onChange={(event) => onInputChange({ region: event.target.value })}
            />
          </label>
          <div className="flex items-center justify-between gap-4 pt-3 [&>div]:flex [&>div]:flex-wrap [&>div]:items-center [&>div]:gap-2 [&_.secondary]:mt-0 [&_[role=status]]:text-xs [&_[role=status]]:text-muted [&_[role=alert]]:text-xs [&_[role=alert]]:text-muted">
            <div>
              <button
                type="button"
                className="secondary"
                disabled={busy || (!stored && (!input.secretId.trim() || !input.secretKey.trim()))}
                onClick={() =>
                  onSave({
                    ...(input.secretId.trim() ? { secretId: input.secretId } : {}),
                    ...(input.secretKey.trim() ? { secretKey: input.secretKey } : {}),
                    region,
                  })
                }
              >
                保存凭据
              </button>
              {stored && (
                <button type="button" className="secondary" disabled={busy} onClick={onClear}>
                  清除凭据
                </button>
              )}
              <CredentialStatusMessage message={message} />
            </div>
          </div>
          {children}
        </>
      )}
    </div>
  );
}
