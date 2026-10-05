import { accountMessage, isAccountCancellation } from "./account-errors";
import { runAsyncAction, type AsyncActionState } from "../core/async-action";
import { type MutableRefObject } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";

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

export interface AccountActionState {
  busy: boolean;
  mounted: MutableRefObject<boolean>;
  clientGeneration: MutableRefObject<number>;
  perform: (
    operation: () => Promise<void>,
    options?: { allowBusy?: boolean },
  ) => Promise<void> | undefined;
}

/** Shares busy, client-generation, and late-result protection across account surfaces. */
export function useAccountAction(
  client: unknown,
  setError: (message: string) => void,
  setNotice: (message: string) => void,
  ...owners: readonly unknown[]
): AccountActionState {
  const {
    busy,
    mounted,
    generation: clientGeneration,
    run,
  } = useAsyncActionRunner(setError, setNotice, client, ...owners);
  const perform: AccountActionState["perform"] = (operation, options = {}) => {
    if (busy && !options.allowBusy) return undefined;
    return run(() => operation(), {
      formatError: accountMessage,
      ignoreError: isAccountCancellation,
    });
  };

  return { busy, mounted, clientGeneration, perform };
}
