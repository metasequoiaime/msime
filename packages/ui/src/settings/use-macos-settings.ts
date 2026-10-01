import { useCallback, useEffect, useRef, useState } from "react";
import type { InputSourceStartupStatus, SettingsClient } from "../index";
import { inputSourceNeedsAdding } from "./input-source-startup-notice";

/** How often the settings page reads the input source list again while the notice waits for the user to act in System Settings or Finder; a focus change reads it at once. */
export const INPUT_SOURCE_RECHECK_MS = 3000;

export interface UseMacosSettingsOptions {
  client: Pick<
    SettingsClient,
    | "inputSourceStartup"
    | "onDeviceTranslation"
    | "loadMacosShuangpinKeymap"
    | "loadMacosWubiAutoCommitUnique"
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
  const [shuangpinKeymap, setShuangpinKeymap] = useState<boolean>();
  const [savedShuangpinKeymap, setSavedShuangpinKeymap] = useState<boolean>();
  const [wubiAutoCommitUnique, setWubiAutoCommitUnique] = useState<boolean>();
  const [savedWubiAutoCommitUnique, setSavedWubiAutoCommitUnique] = useState<boolean>();

  // Set once the user dismisses the notice, so a later focus refresh does not bring it back in this window.
  const inputSourceDismissed = useRef(false);
  const inputSourceRequest = useRef(0);
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
    window.addEventListener("focus", refresh);
    return () => {
      inputSourceRequest.current++;
      window.removeEventListener("focus", refresh);
    };
  }, [client, macos, refreshInputSourceStartup]);

  // System Settings may sit beside the window rather than in front of it, so focus alone can miss the moment the source is added; while the notice is waiting on the user, read again on a short interval as well.
  const waitingOnUser =
    inputSourceNeedsAdding(inputSourceStartup) ||
    Boolean(inputSourceStartup?.system_bundles?.length);
  useEffect(() => {
    if (!waitingOnUser || typeof window === "undefined") return;
    const timer = window.setInterval(
      () => void refreshInputSourceStartup(),
      INPUT_SOURCE_RECHECK_MS,
    );
    return () => window.clearInterval(timer);
  }, [waitingOnUser, refreshInputSourceStartup]);

  // The input method records missing pairs when it next translates, so read again whenever the
  // window comes back, typically from System Settings after a download.
  useEffect(() => {
    const onDeviceTranslation = client.onDeviceTranslation;
    if (!macos || !onDeviceTranslation || typeof window === "undefined") {
      setOnDeviceDownloadable([]);
      return;
    }
    let active = true;
    const refresh = () =>
      void onDeviceTranslation
        .downloadableLanguages()
        .then((codes) => {
          if (active) setOnDeviceDownloadable(codes);
        })
        .catch(() => {
          if (active) setOnDeviceDownloadable([]);
        });
    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      active = false;
      window.removeEventListener("focus", refresh);
    };
  }, [client, macos]);

  useEffect(() => {
    if (!macos || !client.loadMacosShuangpinKeymap) {
      setShuangpinKeymap(undefined);
      setSavedShuangpinKeymap(undefined);
      return;
    }
    let active = true;
    void client
      .loadMacosShuangpinKeymap()
      .then((value) => {
        if (!active) return;
        setShuangpinKeymap(value);
        setSavedShuangpinKeymap(value);
      })
      .catch(() => {
        if (active) setError("无法读取双拼键位提示设置，请重试。");
      });
    return () => {
      active = false;
    };
  }, [client, macos]);

  useEffect(() => {
    if (!macos || !client.loadMacosWubiAutoCommitUnique) {
      setWubiAutoCommitUnique(undefined);
      setSavedWubiAutoCommitUnique(undefined);
      return;
    }
    let active = true;
    void client
      .loadMacosWubiAutoCommitUnique()
      .then((value) => {
        if (!active) return;
        setWubiAutoCommitUnique(value);
        setSavedWubiAutoCommitUnique(value);
      })
      .catch(() => {
        if (active) setError("无法读取五笔自动上屏设置，请重试。");
      });
    return () => {
      active = false;
    };
  }, [client, macos]);

  return {
    dismissInputSourceStartup,
    inputSourceStartup,
    onDeviceDownloadable,
    refreshInputSourceStartup,
    savedShuangpinKeymap,
    savedWubiAutoCommitUnique,
    setSavedShuangpinKeymap,
    setSavedWubiAutoCommitUnique,
    setShuangpinKeymap,
    setWubiAutoCommitUnique,
    shuangpinKeymap,
    wubiAutoCommitUnique,
  } as const;
}
