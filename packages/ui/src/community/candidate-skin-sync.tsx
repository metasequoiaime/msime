import { useEffect, useRef, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import type {
  CandidateSkinCommunityClient,
  CandidateSkinSyncReport,
} from "./community-candidate-skins";

export type CandidateSkinSyncState = {
  busy: boolean;
  report: CandidateSkinSyncReport | null;
  error: unknown;
  /** Starts a run, or queues one behind the run in progress so a change made meanwhile is not missed. */
  run: () => void;
};

/**
 * Keeps the local skin directory and the signed-in user's library in step: once as the page opens, and again whenever `run` is called. `onChanged` follows a run that downloaded or removed a local package, so a listing of the directory can scan again.
 */
export function useCandidateSkinSync(
  client: CandidateSkinCommunityClient | undefined,
  onChanged: () => void,
): CandidateSkinSyncState {
  const [report, setReport] = useState<CandidateSkinSyncReport | null>(null);
  const [error, setError] = useState<unknown>(null);
  const queued = useRef(false);
  const changed = useRef(onChanged);
  changed.current = onChanged;
  const {
    busy,
    mounted,
    running,
    generation: clientGeneration,
    run: runAsyncAction,
  } = useAsyncActionRunner(() => {}, undefined, client);

  const start = async () => {
    if (!client) return;
    queued.current = false;
    const current = clientGeneration.current;
    const action = runAsyncAction(
      async (isCurrent) => {
        try {
          const result = await client.sync();
          if (!isCurrent()) return;
          setReport(result);
          setError(null);
          if (result.downloaded.length || result.deleted_local.length) changed.current();
        } catch (failure) {
          if (mounted.current && current === clientGeneration.current) setError(failure);
        }
      },
      { formatError: () => "" },
    );
    if (!action) return;
    await action;
    if (mounted.current && current === clientGeneration.current && queued.current) void start();
  };

  useEffect(() => {
    queued.current = false;
    setReport(null);
    setError(null);
    void start();
  }, [client, clientGeneration]);

  const run = () => {
    if (running.current) queued.current = true;
    else void start();
  };

  return { busy, report, error, run };
}
