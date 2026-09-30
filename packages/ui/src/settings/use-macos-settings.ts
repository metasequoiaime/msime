import { useEffect, useState } from "react";
import type { InputSourceStartupStatus, SettingsClient } from "../index";

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

  // The Windows installer registers the input method on every install and upgrade; on macOS the
  // settings app does it when it starts, and this tells the user what happened and whether the
  // source still has to be enabled in System Settings.
  useEffect(() => {
    if (!macos || !client.inputSourceStartup) {
      setInputSourceStartup(null);
      return;
    }
    let active = true;
    void client.inputSourceStartup
      .status()
      .then((value) => {
        if (active) setInputSourceStartup(value);
      })
      .catch(() => {
        if (active) setInputSourceStartup(null);
      });
    return () => {
      active = false;
    };
  }, [client, macos]);

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
    inputSourceStartup,
    onDeviceDownloadable,
    savedShuangpinKeymap,
    savedWubiAutoCommitUnique,
    setInputSourceStartup,
    setSavedShuangpinKeymap,
    setSavedWubiAutoCommitUnique,
    setShuangpinKeymap,
    setWubiAutoCommitUnique,
    shuangpinKeymap,
    wubiAutoCommitUnique,
  } as const;
}
