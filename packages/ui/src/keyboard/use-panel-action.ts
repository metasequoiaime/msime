import { useCallback, type MutableRefObject } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";

export type PanelAction = (revision: number) => Promise<void>;

export interface PanelActionState {
  busy: boolean;
  busyRef: MutableRefObject<boolean>;
  revisionRef: MutableRefObject<number>;
  run: (action: PanelAction, failure: string) => Promise<void> | undefined;
  invalidate: () => void;
  isCurrent: (revision: number) => boolean;
}

/** Serializes panel actions and makes late async results harmless after a client is replaced. */
export function usePanelAction(onFailure: (message: string) => void): PanelActionState {
  const noop = useCallback(() => {}, []);
  const {
    busy,
    mounted,
    running: busyRef,
    generation: revisionRef,
    invalidate,
    run: runAsyncAction,
  } = useAsyncActionRunner(noop, undefined);

  const isCurrent = useCallback(
    (revision: number) => mounted.current && revision === revisionRef.current,
    [mounted, revisionRef],
  );

  const run = useCallback(
    (action: PanelAction, failure: string) => {
      if (!mounted.current || busyRef.current) return undefined;
      const revision = ++revisionRef.current;
      return runAsyncAction(() => action(revision), {
        formatError: () => failure,
        onError: () => onFailure(failure),
      });
    },
    [busyRef, mounted, onFailure, revisionRef, runAsyncAction],
  );

  return { busy, busyRef, revisionRef, run, invalidate, isCurrent };
}
