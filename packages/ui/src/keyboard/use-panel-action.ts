import { useCallback, useEffect, useRef, useState, type MutableRefObject } from "react";

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
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const revisionRef = useRef(0);
  const mounted = useRef(true);

  const isCurrent = useCallback(
    (revision: number) => mounted.current && revision === revisionRef.current,
    [],
  );

  const invalidate = useCallback(() => {
    revisionRef.current++;
    busyRef.current = false;
    setBusy(false);
  }, []);

  const run = useCallback(
    (action: PanelAction, failure: string) => {
      if (!mounted.current || busyRef.current) return undefined;
      const revision = ++revisionRef.current;
      busyRef.current = true;
      setBusy(true);
      return (async () => {
        try {
          await action(revision);
        } catch {
          if (isCurrent(revision)) onFailure(failure);
        } finally {
          if (isCurrent(revision)) {
            busyRef.current = false;
            setBusy(false);
          }
        }
      })();
    },
    [isCurrent, onFailure],
  );

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      revisionRef.current++;
      busyRef.current = false;
    };
  }, []);

  return { busy, busyRef, revisionRef, run, invalidate, isCurrent };
}
