export interface CredentialStatusMessageValue {
  ok: boolean;
  text: string;
}

export interface CredentialStatusMessageProps {
  message?: CredentialStatusMessageValue;
}

/** Inline success or error announcement for credential operations. */
export function CredentialStatusMessage({ message }: CredentialStatusMessageProps) {
  return message ? <span role={message.ok ? "status" : "alert"}>{message.text}</span> : null;
}
