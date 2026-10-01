import { useEffect, useRef, useState } from "react";
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
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<CandidateSkinSyncReport | null>(null);
  const [error, setError] = useState<unknown>(null);
  const generation = useRef(0);
  const running = useRef(false);
  const queued = useRef(false);
  const changed = useRef(onChanged);
  changed.current = onChanged;

  const start = async (current: number) => {
    if (!client) return;
    running.current = true;
    queued.current = false;
    setBusy(true);
    try {
      const result = await client.sync();
      if (current !== generation.current) return;
      setReport(result);
      setError(null);
      if (result.downloaded.length || result.deleted_local.length) changed.current();
    } catch (failure) {
      if (current !== generation.current) return;
      setError(failure);
    } finally {
      if (current === generation.current) {
        running.current = false;
        if (queued.current) void start(current);
        else setBusy(false);
      }
    }
  };

  useEffect(() => {
    const current = ++generation.current;
    running.current = false;
    queued.current = false;
    setReport(null);
    setError(null);
    setBusy(false);
    void start(current);
    return () => {
      generation.current++;
    };
  }, [client]);

  const run = () => {
    if (running.current) queued.current = true;
    else void start(generation.current);
  };

  return { busy, report, error, run };
}
