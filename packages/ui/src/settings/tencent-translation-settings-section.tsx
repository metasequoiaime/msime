import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { LinuxTencentCredentialsSection } from "./linux-tencent-credentials-section";
import type {
  TencentCredentialInput,
  TencentCredentialSaveInput,
  TencentCredentialStatus,
} from "./tencent-credential-model";
import { TencentTranslationSection } from "./tencent-translation-section";
import type { CredentialStatusMessageValue } from "./credential-status-message";
import type { TranslationOfflineFallback } from "./translation-service-selector-section";

export interface TencentTranslationSettingsSectionProps {
  grouped?: boolean;
  linux: boolean;
  available: boolean;
  linuxCredentialsAvailable: boolean;
  secretId: string;
  secretKey: string;
  region: string;
  credentialIssue?: string;
  showMissingCredentialsWarning?: boolean;
  offlineFallback?: TranslationOfflineFallback;
  /** 见 `TencentTranslationSectionProps.foldRegion`；Linux 的凭据表单不受影响。 */
  foldRegion?: boolean;
  status?: TencentCredentialStatus;
  input: TencentCredentialInput;
  busy: boolean;
  message?: CredentialStatusMessageValue;
  onSecretIdChange: (value: string) => void;
  onSecretKeyChange: (value: string) => void;
  onRegionChange: (value: string) => void;
  onInputChange?: (patch: Partial<TencentCredentialInput>) => void;
  onSave?: (credential: TencentCredentialSaveInput) => void;
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
  secretId,
  secretKey,
  region,
  credentialIssue,
  showMissingCredentialsWarning = false,
  offlineFallback,
  foldRegion = true,
  status,
  input,
  busy,
  message,
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
      available={available}
      secretId={secretId}
      secretKey={secretKey}
      region={region}
      credentialIssue={credentialIssue}
      showMissingCredentialsWarning={showMissingCredentialsWarning}
      offlineFallback={offlineFallback}
      foldRegion={foldRegion}
      onSecretIdChange={onSecretIdChange}
      onSecretKeyChange={onSecretKeyChange}
      onRegionChange={onRegionChange}
    >
      {serviceCredentialTest}
    </TencentTranslationSection>
  );

  return grouped ? <GroupList title="腾讯云机器翻译">{content}</GroupList> : content;
}
