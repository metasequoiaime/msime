import { useEffect, useState } from "react";
import type { SettingsClient } from "../index";

export interface UseWindowStateOptions {
  client: Pick<SettingsClient, "onWindowStateChanged">;
  setError: (error: string) => void;
}

/** Tracks the native window maximized state while keeping subscription cleanup symmetric. */
export function useWindowState({ client, setError }: UseWindowStateOptions) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    setMaximized(false);
    const subscribe = client.onWindowStateChanged;
    if (subscribe) {
      void Promise.resolve()
        .then(() => {
          if (!active) return;
          return subscribe(
            (value) => {
              if (active) setMaximized(value);
            },
            () => {
              if (active) setError("无法读取窗口状态，请重试。");
            },
          );
        })
        .then((value) => {
          if (active) unsubscribe = value;
          else value?.();
        })
        .catch(() => {
          if (active) setError("无法读取窗口状态，请重试。");
        });
    }
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [client]);

  return maximized;
}
