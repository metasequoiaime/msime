import { type CredentialStatusMessageValue } from "./credential-status-message";
import { CredentialActions } from "./credential-actions";
import { SecretSettingField } from "./secret-setting-field";
import { SettingsManagerBlock } from "./settings-manager-block";

import {
  aiCredentialEditorModel,
  type AiCredentialEditorModelOptions,
} from "./ai-credential-model";

export type { AiCredentialStored } from "./ai-credential-model";

export interface AiCredentialSectionProps extends AiCredentialEditorModelOptions {
  message?: CredentialStatusMessageValue;
  onTokenChange: (token: string) => void;
  onSave: () => void;
  onClear: () => void;
}

/** Linux AI 辅助服务的用户凭据编辑界面。 */
export function AiCredentialSection({
  endpoint,
  model,
  origin,
  token,
  stored,
  invalid,
  busy,
  message,
  onTokenChange,
  onSave,
  onClear,
}: AiCredentialSectionProps) {
  const { status, description, tokenDisabled, saveDisabled, clearDisabled, hasStoredCredential } =
    aiCredentialEditorModel({ endpoint, model, origin, token, stored, invalid, busy });
  return (
    <SettingsManagerBlock role="group" aria-label="AI 凭据">
      <SecretSettingField
        label="API Token"
        inputLabel="AI API Token"
        value={token}
        description={description}
        detail={status}
        disabled={tokenDisabled}
        onChange={onTokenChange}
      />
      <CredentialActions
        saveDisabled={saveDisabled}
        clearDisabled={clearDisabled}
        hasStoredCredential={hasStoredCredential}
        message={message}
        onSave={onSave}
        onClear={onClear}
      />
    </SettingsManagerBlock>
  );
}
