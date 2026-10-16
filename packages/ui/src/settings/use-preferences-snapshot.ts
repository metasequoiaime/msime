import { useEffect, useState } from "react";
import { trackAsyncSubscription } from "../core/async-subscription";
import type { SettingsClient } from "../core/settings-client";
import type { Snapshot } from "../core/preferences-model";
import { useAsyncGeneration } from "./use-async-generation";

export type PreferencesSnapshotClient = Pick<SettingsClient, "load" | "onPreferencesChanged">;

/** 订阅完成后加载偏好，忽略不比已采纳快照更新的事件。 */
export function usePreferencesSnapshot(
  preferences: PreferencesSnapshotClient,
  resetOnClientChange = false,
): Snapshot | null {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const generation = useAsyncGeneration(preferences, resetOnClientChange);

  useEffect(() => {
    const requestGeneration = generation.current;
    const isCurrent = () => generation.current === requestGeneration;
    let latestRevision = -1;
    if (resetOnClientChange) setSnapshot(null);

    const apply = (value: Snapshot) => {
      if (generation.current !== requestGeneration || value.revision <= latestRevision) return;
      latestRevision = value.revision;
      setSnapshot(value);
    };

    const load = async () => {
      if (!isCurrent()) return;
      try {
        apply(await preferences.load());
      } catch {
        // 首次加载失败时保留已采纳的快照。
      }
    };
    let pending: Promise<(() => void) | void>;
    try {
      pending = Promise.resolve(preferences.onPreferencesChanged?.(apply));
    } catch {
      // 同步注册失败时沿用立即开始加载的时机。
      void load();
      return;
    }
    const dispose = trackAsyncSubscription(pending, isCurrent);
    void pending.then(load, load);
    return dispose;
  }, [preferences, resetOnClientChange, generation]);

  return snapshot;
}
