import { useCallback, useEffect, useRef, useState } from "react";
import type { SettingsClient } from "../core/settings-client";
import {
  inputSourceNeedsRecheck,
  type InputSourceStartupStatus,
} from "./input-source-startup-model";
import { useAsyncGeneration } from "./use-async-generation";
import { subscribeToWindowFocus } from "../core/window-focus";
import { useInterval } from "../core/use-interval";

/** How often the settings page reads the input source list again while the notice waits for the user to act in System Settings or Finder; a focus change reads it at once. */
export const INPUT_SOURCE_RECHECK_MS = 3000;

export interface UseMacosSettingsOptions {
  client: Pick<
    SettingsClient,
    "inputSourceStartup" | "onDeviceTranslation" | "loadMacosShuangpinKeymap"
  >;
  macos: boolean;
  setError: (error: string) => void;
}

/** Owns macOS-only native preference reads and focus refreshes. */
export function useMacosSettings({ client, macos, setError }: UseMacosSettingsOptions) {
  const [inputSourceStartup, setInputSourceStartup] = useState<InputSourceStartupStatus | null>(
    null,
  );
  const [onDeviceDownloadable, setOnDeviceDownloadable] = useState<string[]>([]);
  // 双拼键位提示的开关现在是共享偏好 `shuangpin_keymap_hint`；文档里还没有这一项时，macOS 输入法仍按本机 defaults 里的旧选择显示键位图，设置页读它只为显示同一个值。
  const [shuangpinKeymap, setShuangpinKeymap] = useState<boolean>();
  const clientGeneration = useAsyncGeneration(client, macos);
  const onDeviceRequest = useAsyncGeneration(client, macos);

  // Set once the user dismisses the notice, so a later focus refresh does not bring it back in this window.
  const inputSourceDismissed = useRef(false);
  const inputSourceRequest = useAsyncGeneration(client, macos);
  const refreshInputSourceStartup = useCallback(async () => {
    const startup = macos ? client.inputSourceStartup : undefined;
    const request = ++inputSourceRequest.current;
    if (!startup || inputSourceDismissed.current) {
      setInputSourceStartup(null);
      return;
    }
    const value = await startup.status().catch(() => null);
    // Only the newest request answers: a slow start-time read must not overwrite a later one taken after the user enabled the source.
    if (request === inputSourceRequest.current && !inputSourceDismissed.current) {
      setInputSourceStartup(value);
    }
  }, [client, macos]);
  const dismissInputSourceStartup = useCallback(() => {
    inputSourceDismissed.current = true;
    inputSourceRequest.current++;
    setInputSourceStartup(null);
  }, []);

  // The Windows installer registers the input method on every install and upgrade; on macOS the settings app does it when it starts, and this tells the user what happened and whether the source still has to be enabled. Reading again whenever the window comes back, typically from System Settings, lets the notice go away once the user has added the source there.
  useEffect(() => {
    if (!macos || !client.inputSourceStartup || typeof window === "undefined") {
      inputSourceRequest.current++;
      setInputSourceStartup(null);
      return;
    }
    const refresh = () => void refreshInputSourceStartup();
    refresh();
    const stopFocus = subscribeToWindowFocus(refresh);
    return () => {
      inputSourceRequest.current++;
      stopFocus();
    };
  }, [client, macos, refreshInputSourceStartup]);

  // 系统设置可能与本窗口并排，添加输入源不一定带来焦点变化；提示等待用户操作时也定期重查。
  const waitingOnUser = inputSourceNeedsRecheck(inputSourceStartup);
  useInterval(
    () => void refreshInputSourceStartup(),
    waitingOnUser && typeof window !== "undefined" ? INPUT_SOURCE_RECHECK_MS : null,
  );

  // The input method records missing pairs when it next translates, so read again whenever the
  // window comes back, typically from System Settings after a download.
  useEffect(() => {
    const onDeviceTranslation = client.onDeviceTranslation;
    if (!macos || !onDeviceTranslation || typeof window === "undefined") {
      setOnDeviceDownloadable([]);
      return;
    }
    const refresh = () => {
      const generation = ++onDeviceRequest.current;
      void onDeviceTranslation
        .downloadableLanguages()
        .then((codes) => {
          if (generation === onDeviceRequest.current) setOnDeviceDownloadable(codes);
        })
        .catch(() => {
          if (generation === onDeviceRequest.current) setOnDeviceDownloadable([]);
        });
    };
    refresh();
    const stopFocus = subscribeToWindowFocus(refresh);
    return () => {
      stopFocus();
    };
  }, [client, macos, onDeviceRequest]);

  useEffect(() => {
    if (!macos || !client.loadMacosShuangpinKeymap) {
      setShuangpinKeymap(undefined);
      return;
    }
    const generation = clientGeneration.current;
    void client
      .loadMacosShuangpinKeymap()
      .then((value) => {
        if (generation !== clientGeneration.current) return;
        setShuangpinKeymap(value);
      })
      .catch(() => {
        if (generation === clientGeneration.current) setError("无法读取双拼键位提示设置，请重试。");
      });
  }, [client, clientGeneration, macos]);

  return {
    dismissInputSourceStartup,
    inputSourceStartup,
    onDeviceDownloadable,
    refreshInputSourceStartup,
    shuangpinKeymap,
  } as const;
}
