import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useMountedRef } from "../settings/use-mounted-ref";
import { runAsyncAction, type AsyncActionOptions } from "./async-action";

export type AsyncActionOperation = (isCurrent: () => boolean) => Promise<void>;

export interface AsyncActionRunner {
  busy: boolean;
  running: MutableRefObject<boolean>;
  generation: MutableRefObject<number>;
  run: (operation: AsyncActionOperation, options: AsyncActionOptions) => Promise<void> | undefined;
}

/** Shares the busy, owner-generation, and mounted guards around an async UI action. */
export function useAsyncActionRunner(
  setError: (message: string) => void,
  setNotice: ((message: string) => void) | undefined,
  ...owners: readonly unknown[]
): AsyncActionRunner {
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const mounted = useMountedRef();
  const generation = useAsyncGeneration(...owners);

  useEffect(() => {
    running.current = false;
    setBusy(false);
    return () => {
      running.current = false;
    };
  }, owners);

  function run(operation: AsyncActionOperation, options: AsyncActionOptions) {
    if (running.current || !mounted.current) return undefined;
    running.current = true;
    const current = generation.current;
    const promise = runAsyncAction(
      {
        busy: false,
        isCurrent: () => mounted.current && current === generation.current,
        setBusy: (value) => {
          running.current = value;
          setBusy(value);
        },
        setError,
        setNotice,
      },
      operation,
      options,
    );
    return promise.finally(() => {
      if (current === generation.current) running.current = false;
    });
  }

  return { busy, running, generation, run };
}
