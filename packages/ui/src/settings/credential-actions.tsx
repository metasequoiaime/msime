import {
  CredentialStatusMessage,
  type CredentialStatusMessageValue,
} from "./credential-status-message";
import * as settings from "./settings-style";

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
    <div className={settings.serviceRow}>
      <div>
        <button
          type="button"
          className="secondary"
          aria-label={saveAriaLabel}
          disabled={saveDisabled}
          onClick={onSave}
        >
          保存凭据
        </button>
        {hasStoredCredential && (
          <button
            type="button"
            className="secondary"
            aria-label={clearAriaLabel}
            disabled={clearDisabled}
            onClick={onClear}
          >
            清除凭据
          </button>
        )}
        <CredentialStatusMessage message={message} />
      </div>
    </div>
  );
}
