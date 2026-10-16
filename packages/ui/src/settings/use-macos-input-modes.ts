import { useCallback, useEffect, useState } from "react";
import type { EditionInfo } from "../core/host-capabilities";
import type { InputScheme } from "../core/input-scheme";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useInterval } from "../core/use-interval";
import { useLatestRef } from "../core/use-latest-ref";
import { subscribeToWindowFocus } from "../core/window-focus";
import {
  groupMacosInputModeEntries,
  INPUT_MODE_RECHECK_MS,
  INPUT_MODE_RECHECK_WINDOW_MS,
  missingMacosInputModeEntries,
  type MacosInputModeEntry,
  type MacosInputModeGroup,
  type MacosInputModesClient,
} from "./macos-input-modes";
import { useAsyncGeneration } from "./use-async-generation";

export interface UseMacosInputModesOptions {
  client?: MacosInputModesClient;
  /** 当前草稿的输入方案；优先提示它尚未加入的入口。 */
  scheme: InputScheme;
  /** 宿主实际提供的方案，不列没有安装的方案入口。 */
  inputSchemes: readonly InputScheme[];
  /** 运行中的版别，缺省时使用完整版本。 */
  edition?: EditionInfo;
  onError: (message: string) => void;
}

export interface MacosInputModesState {
  enabled: readonly string[] | null;
  missing: readonly MacosInputModeEntry[];
  current: MacosInputModeEntry | undefined;
  groups: readonly MacosInputModeGroup[];
  opening: boolean;
  refresh: () => Promise<void>;
  openSettings: () => Promise<void> | undefined;
}

/** 入口列表读取、焦点刷新与打开系统设置后的限时轮询；客户端变化隔离旧状态和动作。 */
export function useMacosInputModes(options: UseMacosInputModesOptions): MacosInputModesState {
  const { client, scheme, inputSchemes, edition } = options;
  const latest = useLatestRef(options);
  const [enabled, setEnabled] = useState<readonly string[] | null>(null);
  const [watchingUntil, setWatchingUntil] = useState(0);
  const request = useAsyncGeneration(client);
  // runner 开始动作时清空内部错误；展示回调沿用只通知失败的契约。
  const reportError = useCallback(
    (message: string) => {
      if (message) latest.current.onError(message);
    },
    [latest],
  );
  const { run, busy: opening } = useAsyncActionRunner(reportError, undefined, client);
  const refresh = useCallback(async () => {
    if (!client) return;
    const current = ++request.current;
    let value: readonly string[] | null;
    try {
      value = await client.enabled();
    } catch {
      value = null;
    }
    if (current === request.current) setEnabled(value);
  }, [client, request]);

  useEffect(() => {
    setEnabled(null);
    setWatchingUntil(0);
    if (!client || typeof window === "undefined") return;
    void refresh();
    return subscribeToWindowFocus(refresh);
  }, [client, refresh]);

  useInterval(
    () => {
      if (Date.now() >= watchingUntil) {
        setWatchingUntil(0);
        return;
      }
      void refresh();
    },
    watchingUntil ? INPUT_MODE_RECHECK_MS : null,
  );

  const openSettings = () => {
    if (!client) return;
    return run(
      async () => {
        setWatchingUntil(Date.now() + INPUT_MODE_RECHECK_WINDOW_MS);
        await client.openSettings();
      },
      {
        formatError: () => "无法打开系统设置，请手动前往「系统设置 › 键盘 › 文字输入 › 输入法」。",
      },
    );
  };
  const missing = enabled ? missingMacosInputModeEntries(enabled, inputSchemes, edition) : [];
  return {
    enabled,
    missing,
    current: missing.find((entry) => entry.scheme === scheme),
    groups: groupMacosInputModeEntries(missing),
    opening,
    refresh,
    openSettings,
  };
}
