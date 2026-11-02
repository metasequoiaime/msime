import type { ReactNode } from "react";
import { CredentialActions } from "./credential-actions";
import { SettingSectionTitle } from "./setting-section-title";
import { type CredentialStatusMessageValue } from "./credential-status-message";
import { PasswordSettingField } from "./password-setting-field";
import { TextSettingField } from "./text-setting-field";
import { SettingsInputDescription } from "./settings-input-description";
import { MoreOptions } from "../core/platform-controls";
import {
  tencentCredentialEditorModel,
  tencentCredentialSavePayload,
  type TencentCredentialInput,
  type TencentCredentialSaveInput,
  type TencentCredentialStatus,
} from "./tencent-credential-model";

export type {
  TencentCredentialInput as LinuxTencentCredentialInput,
  TencentCredentialStatus as LinuxTencentCredentialStatus,
} from "./tencent-credential-model";

export interface LinuxTencentCredentialsSectionProps {
  available: boolean;
  status?: TencentCredentialStatus;
  input: TencentCredentialInput;
  busy: boolean;
  message?: CredentialStatusMessageValue;
  onInputChange: (patch: Partial<TencentCredentialInput>) => void;
  onSave: (credential: TencentCredentialSaveInput) => void;
  onClear: () => void;
  children?: ReactNode;
}

/** Linux 腾讯云翻译凭据表单；已存状态只提供地域，不回填密钥。 */
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
  const {
    region,
    stored,
    status: credentialStatus,
    description,
    regionInvalid,
    saveDisabled,
    clearDisabled,
  } = tencentCredentialEditorModel({ status, input, busy });

  return (
    <div className="section" role="group" aria-label="在线翻译服务">
      <SettingSectionTitle as="div" title="在线翻译服务" description={description} />
      {!available ? (
        <SettingsInputDescription>
          候选词翻译开启后，provider 从用户配置目录的 <code>tencent-provider.json</code>{" "}
          读取腾讯云凭据；设置页不保存不会生效的 SecretId 或 SecretKey。
        </SettingsInputDescription>
      ) : (
        <>
          {credentialStatus && (
            <SettingsInputDescription>{credentialStatus}</SettingsInputDescription>
          )}
          <PasswordSettingField
            label="SecretId"
            inputLabel="腾讯云 SecretId"
            value={input.secretId}
            onChange={(secretId) => onInputChange({ secretId })}
          />
          <PasswordSettingField
            label="SecretKey"
            inputLabel="腾讯云 SecretKey"
            value={input.secretKey}
            onChange={(secretKey) => onInputChange({ secretKey })}
          />
          {/* 与共享的 TencentTranslationSection 相同：provider 一律请求 tmt.tencentcloudapi.com，地域只写进 X-TC-Region，留空按广州处理（msime-linux-online-provider），平时用不着改，收进「更多选项」；值不合规时展开。地域仍在 input 里，「保存」照样带上它。 */}
          <MoreOptions defaultOpen={regionInvalid}>
            <TextSettingField
              label="地域"
              inputLabel="腾讯云地域"
              value={region}
              onChange={(value) => onInputChange({ region: value })}
            />
          </MoreOptions>
          <CredentialActions
            saveDisabled={saveDisabled}
            clearDisabled={clearDisabled}
            hasStoredCredential={Boolean(stored)}
            message={message}
            onSave={() => onSave(tencentCredentialSavePayload({ status, input }))}
            onClear={onClear}
          />
          {children}
        </>
      )}
    </div>
  );
}
