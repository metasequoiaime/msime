import { accountMessage, isAccountCancellation } from "./account-errors";
import { runAsyncAction, type AsyncActionState } from "../core/async-action";
import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useMountedRef } from "../settings/use-mounted-ref";

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
  const [busy, setBusy] = useState(false);
  const mounted = useMountedRef();
  const clientGeneration = useAsyncGeneration(client, ...owners);
  const actionRunning = useRef(false);

  useEffect(() => {
    actionRunning.current = false;
    setBusy(false);
    return () => {
      actionRunning.current = false;
    };
  }, [client, ...owners]);

  const perform = (operation: () => Promise<void>, options: { allowBusy?: boolean } = {}) => {
    if (actionRunning.current || (busy && !options.allowBusy)) return undefined;
    const generation = clientGeneration.current;
    actionRunning.current = true;
    return runAccountOperation(
      {
        busy: options.allowBusy ? false : busy,
        isCurrent: () => mounted.current && generation === clientGeneration.current,
        setBusy,
        setError,
        setNotice,
      },
      operation,
    ).finally(() => {
      if (generation === clientGeneration.current) actionRunning.current = false;
    });
  };

  return { busy, mounted, clientGeneration, perform };
}
