import {
  CredentialStatusMessage,
  type CredentialStatusMessageValue,
} from "./credential-status-message";
import { SettingsServiceRow } from "./settings-service-row";
import { ActionButton } from "./action-button";

export interface CredentialActionsProps {
  saveDisabled: boolean;
  clearDisabled: boolean;
  hasStoredCredential: boolean;
  saveAriaLabel?: string;
  clearAriaLabel?: string;
  message?: CredentialStatusMessageValue;
  onSave: () => void;
  onClear: () => void;
}

/** Shared save, clear and status controls for provider credential sections. */
export function CredentialActions({
  saveDisabled,
  clearDisabled,
  hasStoredCredential,
  saveAriaLabel,
  clearAriaLabel,
  message,
  onSave,
  onClear,
}: CredentialActionsProps) {
  return (
    <SettingsServiceRow>
      <div>
        <ActionButton
          action={onSave}
          ariaLabel={saveAriaLabel}
          disabled={saveDisabled}
          label="保存凭据"
        />
        {hasStoredCredential && (
          <ActionButton
            action={onClear}
            ariaLabel={clearAriaLabel}
            disabled={clearDisabled}
            label="清除凭据"
          />
        )}
        <CredentialStatusMessage message={message} />
      </div>
    </SettingsServiceRow>
  );
}
