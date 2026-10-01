import { accountMessage, isAccountCancellation } from "./account-errors";
import { runAsyncAction, type AsyncActionState } from "../core/async-action";

export type AccountOperationState = Omit<AsyncActionState, "setNotice"> & {
  setNotice: (message: string) => void;
};

/** Runs an account action and only publishes its result while its page and client are current. */
export async function runAccountOperation(
  state: AccountOperationState,
  operation: () => Promise<void>,
): Promise<void> {
  return runAsyncAction(state, () => operation(), {
    formatError: accountMessage,
    ignoreError: isAccountCancellation,
  });
}
