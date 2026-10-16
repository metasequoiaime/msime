import { useEffect, useState } from "react";
import type { SettingsClient } from "../core/settings-client";
import { trackAsyncSubscription } from "../core/async-subscription";
import { useMountedRef } from "./use-mounted-ref";
import { useAsyncGeneration } from "./use-async-generation";

export interface UseWindowStateOptions {
  client: Pick<SettingsClient, "onWindowStateChanged">;
  setError: (error: string) => void;
}

/** 跟踪原生窗口最大化状态，异步监听由公共工具释放。 */
export function useWindowState({ client, setError }: UseWindowStateOptions) {
  const [maximized, setMaximized] = useState(false);
  const mounted = useMountedRef();
  const generation = useAsyncGeneration(client);

  useEffect(() => {
    const current = generation.current;
    const isCurrent = () => mounted.current && generation.current === current;
    setMaximized(false);
    const subscribe = client.onWindowStateChanged;
    if (!subscribe) return;
    const pending = Promise.resolve()
      .then(() => {
        if (!isCurrent()) return;
        return subscribe(
          (value) => {
            if (isCurrent()) setMaximized(value);
          },
          () => {
            if (isCurrent()) setError("无法读取窗口状态，请重试。");
          },
        );
      })
      .catch(() => {
        if (isCurrent()) setError("无法读取窗口状态，请重试。");
      });
    return trackAsyncSubscription(pending, isCurrent);
  }, [client, mounted, generation]);

  return maximized;
}
