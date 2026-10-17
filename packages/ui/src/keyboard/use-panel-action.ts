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

/** 串行执行面板动作，并丢弃客户端替换后的迟到结果。 */
export function usePanelAction(
  onFailure: (message: string) => void,
  formatFailure?: (error: unknown, fallback: string) => string,
): PanelActionState {
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
        formatError: (error) => formatFailure?.(error, failure) ?? failure,
        onError: (error) => onFailure(formatFailure?.(error, failure) ?? failure),
      });
    },
    [busyRef, mounted, onFailure, formatFailure, revisionRef, runAsyncAction],
  );

  return { busy, busyRef, revisionRef, run, invalidate, isCurrent };
}
