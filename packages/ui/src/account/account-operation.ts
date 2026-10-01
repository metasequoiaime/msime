import { accountMessage, isAccountCancellation } from "./account-errors";

export interface AccountOperationState {
  busy: boolean;
  isCurrent: () => boolean;
  setBusy: (busy: boolean) => void;
  setError: (message: string) => void;
  setNotice: (message: string) => void;
}

/** Runs an account action and only publishes its result while its page and client are current. */
export async function runAccountOperation(
  { busy, isCurrent, setBusy, setError, setNotice }: AccountOperationState,
  operation: () => Promise<void>,
): Promise<void> {
  if (busy || !isCurrent()) return;
  setBusy(true);
  setError("");
  setNotice("");
  try {
    await operation();
  } catch (error) {
    if (isCurrent() && !isAccountCancellation(error)) setError(accountMessage(error));
  } finally {
    if (isCurrent()) setBusy(false);
  }
}
