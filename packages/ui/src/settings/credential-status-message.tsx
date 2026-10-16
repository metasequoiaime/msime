import type { CredentialStatusMessageValue } from "./credential-status-message-model";

export type { CredentialStatusMessageValue } from "./credential-status-message-model";

export interface CredentialStatusMessageProps {
  message?: CredentialStatusMessageValue;
}

/** Inline success or error announcement for credential operations. */
export function CredentialStatusMessage({ message }: CredentialStatusMessageProps) {
  return message ? <span role={message.ok ? "status" : "alert"}>{message.text}</span> : null;
}
