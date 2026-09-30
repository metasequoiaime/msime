import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import {
  LinuxTencentCredentialsSection,
  type LinuxTencentCredentialInput,
  type LinuxTencentCredentialStatus,
} from "./linux-tencent-credentials-section";
import { TencentTranslationSection } from "./tencent-translation-section";
import type { CredentialStatusMessageValue } from "./credential-status-message";

export interface TencentTranslationSettingsSectionProps {
  grouped?: boolean;
  linux: boolean;
  available: boolean;
  linuxCredentialsAvailable: boolean;
  enabled: boolean;
  secretId: string;
  secretKey: string;
  region: string;
  credentialIssue?: string;
  showMissingCredentialsWarning?: boolean;
  status?: LinuxTencentCredentialStatus;
  input: LinuxTencentCredentialInput;
  busy: boolean;
  message?: CredentialStatusMessageValue;
  onToggle: (enabled: boolean) => void;
  onSecretIdChange: (value: string) => void;
  onSecretKeyChange: (value: string) => void;
  onRegionChange: (value: string) => void;
  onInputChange?: (patch: Partial<LinuxTencentCredentialInput>) => void;
  onSave?: (credential: { secretId?: string; secretKey?: string; region: string }) => void;
  onClear?: () => void;
  linuxCredentialTest?: ReactNode;
  serviceCredentialTest?: ReactNode;
}

/** Shared Tencent translation settings binding for the page and embedded panel hosts. */
export function TencentTranslationSettingsSection({
  grouped = false,
  linux,
  available,
  linuxCredentialsAvailable,
  enabled,
  secretId,
  secretKey,
  region,
  credentialIssue,
  showMissingCredentialsWarning = false,
  status,
  input,
  busy,
  message,
  onToggle,
  onSecretIdChange,
  onSecretKeyChange,
  onRegionChange,
  onInputChange,
  onSave,
  onClear,
  linuxCredentialTest,
  serviceCredentialTest,
}: TencentTranslationSettingsSectionProps) {
  const content = linux ? (
    <LinuxTencentCredentialsSection
      available={linuxCredentialsAvailable}
      status={status}
      input={input}
      busy={busy}
      message={message}
      onInputChange={(patch) => onInputChange?.(patch)}
      onSave={(credential) => onSave?.(credential)}
      onClear={() => onClear?.()}
    >
      {linuxCredentialTest}
    </LinuxTencentCredentialsSection>
  ) : (
    <TencentTranslationSection
      enabled={enabled}
      available={available}
      secretId={secretId}
      secretKey={secretKey}
      region={region}
      credentialIssue={credentialIssue}
      showMissingCredentialsWarning={showMissingCredentialsWarning}
      onToggle={onToggle}
      onSecretIdChange={onSecretIdChange}
      onSecretKeyChange={onSecretKeyChange}
      onRegionChange={onRegionChange}
    >
      {serviceCredentialTest}
    </TencentTranslationSection>
  );

  return grouped ? <GroupList title="腾讯云机器翻译">{content}</GroupList> : content;
}
