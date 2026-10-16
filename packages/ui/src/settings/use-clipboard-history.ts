import { useCallback, useEffect, useRef, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useLatestRef } from "../core/use-latest-ref";
import { useAsyncGeneration } from "./use-async-generation";
import type { ClipboardHistoryClient, ClipboardHistoryEntry } from "./clipboard-history-model";

export interface UseClipboardHistoryOptions {
  client?: ClipboardHistoryClient;
  historyEnabled: boolean;
  persistedHistoryEnabled: boolean;
  revision?: number;
  page?: string;
  ios: boolean;
  onToggle: (enabled: boolean) => void;
  onError: (message: string) => void;
}

export interface ClipboardHistoryState {
  entries: ClipboardHistoryEntry[];
  clearArmed: boolean;
  historyBusy: boolean;
  historyShown: boolean;
  toggle: (enabled: boolean) => void;
  clear: () => Promise<void> | undefined;
  copy: (text: string) => Promise<void>;
  remove: (text: string) => Promise<void> | undefined;
  setPinned: (text: string, pinned: boolean) => Promise<void> | undefined;
}

/** 独立管理历史读取、两步清空和写入；页面只决定何时读取，不取消已发出的写入。 */
export function useClipboardHistory({
  client,
  historyEnabled,
  persistedHistoryEnabled,
  revision,
  page,
  ios,
  onToggle,
  onError,
}: UseClipboardHistoryOptions): ClipboardHistoryState {
  const [entries, setEntries] = useState<ClipboardHistoryEntry[]>([]);
  const [clearArmed, setClearArmed] = useState(false);
  const errorCallback = useLatestRef(onError);
  const reportError = useCallback(
    (message: string) => {
      if (message) errorCallback.current(message);
    },
    [errorCallback],
  );
  const {
    run,
    busy: historyBusy,
    generation: actionGeneration,
    mounted,
  } = useAsyncActionRunner(
    reportError,
    undefined,
    client,
    ios,
    historyEnabled,
    persistedHistoryEnabled,
  );
  const historyGeneration = useAsyncGeneration(
    client,
    ios,
    page,
    historyEnabled,
    persistedHistoryEnabled,
    revision,
  );
  const historyRead = useRef(0);

  useEffect(() => {
    setEntries([]);
    setClearArmed(false);
  }, [client, ios, historyEnabled, persistedHistoryEnabled]);

  useEffect(() => {
    const currentGeneration = historyGeneration.current;
    const currentRead = historyRead.current;
    // 设置页隐藏后仍挂载，返回历史页时再读，避免启动必然失效的请求。
    if (page && page !== "tools") return;
    if ((!ios && !persistedHistoryEnabled) || !client?.list) return;
    void client
      .list()
      .then((next) => {
        if (
          currentGeneration === historyGeneration.current &&
          currentRead === historyRead.current
        ) {
          setEntries(next);
          setClearArmed(false);
        }
      })
      .catch(() => undefined);
  }, [client, historyGeneration, ios, page, historyEnabled, persistedHistoryEnabled, revision]);

  const persistedBefore = useRef(persistedHistoryEnabled);
  useEffect(() => {
    const turnedOn = !persistedBefore.current && persistedHistoryEnabled;
    persistedBefore.current = persistedHistoryEnabled;
    const sync = client?.sync;
    // 只在已保存的开关由关到开时补记当前剪贴板，打开页面不触发。
    if (ios || !turnedOn || !historyEnabled || !sync) return;
    void run(
      async (isCurrent) => {
        const next = await sync();
        if (!isCurrent()) return;
        historyRead.current++;
        setEntries(next);
        setClearArmed(false);
      },
      { formatError: () => "无法同步剪贴板历史" },
    );
  }, [client, ios, historyEnabled, persistedHistoryEnabled, run]);

  const mutate = (action: () => Promise<void>, failure: string) =>
    run(
      async (isCurrent) => {
        await action();
        if (!isCurrent()) return;
        const currentRead = ++historyRead.current;
        setClearArmed(false);
        if (client?.list) {
          const next = await client.list();
          if (isCurrent() && currentRead === historyRead.current) setEntries(next);
        }
      },
      { formatError: () => failure },
    );

  const toggle = (enabled: boolean) => {
    onToggle(enabled);
    if (!enabled) {
      setEntries([]);
      setClearArmed(false);
    }
  };
  const clear = () => {
    if (!client || historyBusy || !historyEnabled) return;
    if (!clearArmed) {
      setClearArmed(true);
      return;
    }
    return mutate(() => client.clear(), "无法清空剪贴板历史，请稍后重试。");
  };
  const copy = async (text: string) => {
    const current = actionGeneration.current;
    try {
      await client?.copy?.(text);
    } catch {
      if (mounted.current && current === actionGeneration.current)
        reportError("无法复制剪贴板记录");
    }
  };

  return {
    entries,
    clearArmed,
    historyBusy,
    historyShown: historyEnabled && Boolean(client?.list),
    toggle,
    clear,
    copy,
    remove: (text) =>
      client?.remove ? mutate(() => client.remove!(text), "无法删除剪贴板记录") : undefined,
    setPinned: (text, pinned) =>
      client?.setPinned
        ? mutate(() => client.setPinned!(text, pinned), "无法更新剪贴板固定状态")
        : undefined,
  };
}
