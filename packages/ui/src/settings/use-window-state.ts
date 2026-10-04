import { useEffect, useState } from "react";
import type { SettingsClient } from "../index";
import { useMountedRef } from "./use-mounted-ref";
import { useAsyncGeneration } from "./use-async-generation";

export interface UseWindowStateOptions {
  client: Pick<SettingsClient, "onWindowStateChanged">;
  setError: (error: string) => void;
}

/** Tracks the native window maximized state while keeping subscription cleanup symmetric. */
export function useWindowState({ client, setError }: UseWindowStateOptions) {
  const [maximized, setMaximized] = useState(false);
  const mounted = useMountedRef();
  const generation = useAsyncGeneration(client);

  useEffect(() => {
    const current = generation.current;
    let unsubscribe: (() => void) | undefined;
    setMaximized(false);
    const subscribe = client.onWindowStateChanged;
    if (subscribe) {
      void Promise.resolve()
        .then(() => {
          if (!mounted.current || generation.current !== current) return;
          return subscribe(
            (value) => {
              if (mounted.current && generation.current === current) setMaximized(value);
            },
            () => {
              if (mounted.current && generation.current === current)
                setError("无法读取窗口状态，请重试。");
            },
          );
        })
        .then((value) => {
          if (mounted.current && generation.current === current) unsubscribe = value;
          else value?.();
        })
        .catch(() => {
          if (mounted.current && generation.current === current)
            setError("无法读取窗口状态，请重试。");
        });
    }
    return () => unsubscribe?.();
  }, [client, mounted]);

  return maximized;
}
