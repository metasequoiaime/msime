import { useEffect, useState } from "react";
import type { SettingsClient, Snapshot } from "../index";

export type PreferencesSnapshotClient = Pick<SettingsClient, "load" | "onPreferencesChanged">;

/** Loads preferences after subscribing, then ignores change events older than the loaded snapshot. */
export function usePreferencesSnapshot(
  preferences: PreferencesSnapshotClient,
  resetOnClientChange = false,
): Snapshot | null {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);

  useEffect(() => {
    let active = true;
    let latestRevision = -1;
    let unsubscribe: (() => void) | undefined;
    if (resetOnClientChange) setSnapshot(null);

    const apply = (value: Snapshot) => {
      if (!active || value.revision <= latestRevision) return;
      latestRevision = value.revision;
      setSnapshot(value);
    };

    const start = async () => {
      try {
        const stop = await preferences.onPreferencesChanged?.(apply);
        if (!active) {
          stop?.();
          return;
        }
        unsubscribe = stop;
      } catch {
        /* Initial loading still works when event subscription is unavailable. */
      }
      if (active) {
        try {
          apply(await preferences.load());
        } catch {
          /* Keep the last available snapshot when the initial load fails. */
        }
      }
    };
    void start();
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [preferences, resetOnClientChange]);

  return snapshot;
}
