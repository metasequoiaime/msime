import { useEffect } from "react";
import type { SettingsClient } from "../core/settings-client";
import { subscribeToInterval } from "../core/use-interval";
import { subscribeToWindowFocus } from "../core/window-focus";

const checkIntervalMs = 60 * 60 * 1000;

/** 设置会话在首次挂载、获得焦点和每小时请求自动检查；每日节流与更新策略仍由共享宿主负责。 */
export function useAutomaticDictionaryUpdates(client: Pick<SettingsClient, "dictionary">): void {
  useEffect(() => {
    const automatic = () => {
      void client.dictionary?.updates?.("automatic").catch(() => undefined);
    };
    automatic();
    const stopFocus = subscribeToWindowFocus(automatic);
    const stopInterval = subscribeToInterval(automatic, checkIntervalMs);
    return () => {
      stopFocus();
      stopInterval();
    };
  }, [client.dictionary?.updates]);
}
