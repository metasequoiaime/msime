import { useCallback, useEffect, useRef, type MutableRefObject } from "react";

export interface CommunityClientLifecycle {
  mounted: MutableRefObject<boolean>;
  clientGeneration: MutableRefObject<number>;
  actionRunning: MutableRefObject<boolean>;
  isCurrent: (generation: number) => boolean;
}

/** Tracks client ownership for asynchronous community views and actions. */
export function useCommunityClientLifecycle(
  client: unknown,
  ...owners: readonly unknown[]
): CommunityClientLifecycle {
  const mounted = useRef(true);
  const clientGeneration = useRef(0);
  const actionRunning = useRef(false);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    mounted.current = true;
    actionRunning.current = false;
    return () => {
      mounted.current = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client, ...owners]);

  const isCurrent = useCallback(
    (generation: number) => mounted.current && generation === clientGeneration.current,
    [],
  );

  return { mounted, clientGeneration, actionRunning, isCurrent };
}
