import { useEffect, useState } from "react";
import type { SettingsClient, Snapshot } from "../index";
import { useAsyncGeneration } from "./use-async-generation";

export type PreferencesSnapshotClient = Pick<SettingsClient, "load" | "onPreferencesChanged">;

/** Loads preferences after subscribing, then ignores change events older than the loaded snapshot. */
export function usePreferencesSnapshot(
  preferences: PreferencesSnapshotClient,
  resetOnClientChange = false,
): Snapshot | null {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const generation = useAsyncGeneration(preferences, resetOnClientChange);

  useEffect(() => {
    const requestGeneration = generation.current;
    let latestRevision = -1;
    let unsubscribe: (() => void) | undefined;
    if (resetOnClientChange) setSnapshot(null);

    const apply = (value: Snapshot) => {
      if (generation.current !== requestGeneration || value.revision <= latestRevision) return;
      latestRevision = value.revision;
      setSnapshot(value);
    };

    const start = async () => {
      try {
        const stop = await preferences.onPreferencesChanged?.(apply);
        if (generation.current !== requestGeneration) {
          stop?.();
          return;
        }
        unsubscribe = stop;
      } catch {
        /* Initial loading still works when event subscription is unavailable. */
      }
      if (generation.current === requestGeneration) {
        try {
          apply(await preferences.load());
        } catch {
          /* Keep the last available snapshot when the initial load fails. */
        }
      }
    };
    void start();
    return () => {
      unsubscribe?.();
    };
  }, [preferences, resetOnClientChange, generation]);

  return snapshot;
}
