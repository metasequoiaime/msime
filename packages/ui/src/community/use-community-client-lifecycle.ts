import { useCallback, useEffect, useRef, type MutableRefObject } from "react";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useMountedRef } from "../settings/use-mounted-ref";

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
  const mounted = useMountedRef();
  const clientGeneration = useAsyncGeneration(client, ...owners);
  const actionRunning = useRef(false);

  useEffect(() => {
    actionRunning.current = false;
    return () => {
      actionRunning.current = false;
    };
  }, [client, ...owners]);

  const isCurrent = useCallback(
    (generation: number) => mounted.current && generation === clientGeneration.current,
    [],
  );

  return { mounted, clientGeneration, actionRunning, isCurrent };
}
