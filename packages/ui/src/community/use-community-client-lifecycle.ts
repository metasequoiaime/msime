import { useCallback, type MutableRefObject } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";

export interface CommunityClientLifecycle {
  mounted: MutableRefObject<boolean>;
  clientGeneration: MutableRefObject<number>;
  actionRunning: MutableRefObject<boolean>;
  isCurrent: (generation: number) => boolean;
}

const ignoreError = (_message: string) => {};

/** Tracks client ownership for asynchronous community views and actions. */
export function useCommunityClientLifecycle(
  client: unknown,
  ...owners: readonly unknown[]
): CommunityClientLifecycle {
  const {
    mounted,
    generation: clientGeneration,
    running: actionRunning,
  } = useAsyncActionRunner(ignoreError, undefined, client, ...owners);

  const isCurrent = useCallback(
    (generation: number) => mounted.current && generation === clientGeneration.current,
    [],
  );

  return { mounted, clientGeneration, actionRunning, isCurrent };
}
