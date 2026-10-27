import { useEffect, useMemo, useRef, useState } from "react";
import type { SettingsClient } from "../core/settings-client";
import type { CloudClipboardDeviceStatusShape } from "../keyboard/cloud-response";
import {
  dictionarySyncDelay,
  dictionarySyncDevices,
  dictionarySyncRun,
  dictionarySyncStatus,
  type DictionarySyncAction,
  type DictionarySyncRun,
  type DictionarySyncStatus,
  type DictionarySyncTrigger,
  type DictionarySyncWebDavPreferences,
} from "./dictionary-sync";
import { errorCode } from "../core/error-code";

/** 经设置改了词的通知：词库客户端的写操作成功后发出，词库同步据此以 `local_change` 记一次本机改动。 */
export interface DictionaryLocalChanges {
  subscribe(listener: () => void): () => void;
  notify(): void;
}

/**
 * 宿主自己往「我的词条」里写进了词、页面并不知道时派发在 `window` 上的事件：鸿蒙设置应用把键盘经共享沙箱发布的词写进自己的词库、或者排空了自己的个人词库队列之后（`platforms/harmony/README.md`「「我的词条」只有一份」）。词库同步收到它就和经设置改了词一样，以 `local_change` 记一次本机改动。
 */
export const HOST_USER_WORDS_CHANGED_EVENT = "msime-user-words-changed";

export function createDictionaryLocalChanges(): DictionaryLocalChanges {
  const listeners = new Set<() => void>();
  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    notify() {
      for (const listener of listeners) listener();
    },
  };
}

type AsyncMethod = (...args: never[]) => Promise<unknown>;

/** 包一层：原方法成功之后发通知，结果和错误原样交回。 */
function notifying<T extends AsyncMethod | undefined>(method: T, notify: () => void): T {
  if (!method) return method;
  return (async (...args: Parameters<NonNullable<T>>) => {
    const result = await method(...args);
    notify();
    return result;
  }) as T;
}

/** 宿主提供词库同步时，给会改动「我的词条」的词库方法（编辑、导入、重试、命名词库的增删改和启停、清除学习数据）加上成功后的通知；不提供时原样返回同一个客户端，其他宿主的行为不变。只读的方法（列表、查询、导出）不包。 */
export function withDictionaryChangeNotices(
  client: SettingsClient,
  notify: () => void,
): SettingsClient {
  if (!client.dictionarySync) return client;
  const { dictionary, dictionaryCollections, resetLearnedData } = client;
  return {
    ...client,
    ...(dictionary
      ? {
          dictionary: {
            ...dictionary,
            edit: notifying(dictionary.edit, notify),
            import: notifying(dictionary.import, notify),
            importPersonal: notifying(dictionary.importPersonal, notify),
            retry: notifying(dictionary.retry, notify),
          },
        }
      : {}),
    ...(dictionaryCollections
      ? {
          dictionaryCollections: {
            ...dictionaryCollections,
            delete: notifying(dictionaryCollections.delete, notify),
            setEnabled: notifying(dictionaryCollections.setEnabled, notify),
            addWords: notifying(dictionaryCollections.addWords, notify),
            importFile: notifying(dictionaryCollections.importFile, notify),
            importBytes: notifying(dictionaryCollections.importBytes, notify),
            installCommunity: notifying(dictionaryCollections.installCommunity, notify),
          },
        }
      : {}),
    ...(resetLearnedData ? { resetLearnedData: notifying(resetLearnedData, notify) } : {}),
  };
}

/** 设置模型里的词库同步：状态、设备和用户的操作。宿主不提供时为 `null`。 */
export interface DictionarySyncModel {
  /** 已保存的偏好里配置了网盘。 */
  configured: boolean;
  /** 最近一次读到的概况；还没读到时为 `null`。 */
  status: DictionarySyncStatus | null;
  /** 最近一次同步或操作失败的错误码；成功后清掉。 */
  error: string | null;
  /** 用户发起的操作（立即同步、设备管理、重置）正在进行。 */
  busy: boolean;
  /** 有一次同步正在进行，包括自动的。 */
  syncing: boolean;
  /** 词库密钥串的设备；还没取过时为 `null`。 */
  devices: CloudClipboardDeviceStatusShape | null;
  /** 正在取设备列表。 */
  devicesLoading: boolean;
  /** 最近一次用户操作的结果说明。 */
  notice: string;
  syncNow(): Promise<void>;
  /** 对被挡下的大批删除的选择：同步删除，或者从网盘恢复。 */
  resolveDeletions(choice: "confirm" | "restore"): Promise<void>;
  /** 取设备列表。`quiet` 是打开页面时自动取的那一次：不占住按钮，失败也不报错，用户可以再点「查看设备」。 */
  loadDevices(quiet?: boolean): Promise<void>;
  approve(deviceId: string): Promise<void>;
  reject(deviceId: string): Promise<void>;
  remove(deviceId: string): Promise<void>;
  reset(): Promise<void>;
  restart(): Promise<void>;
  /** 用户确认网盘现状后恢复；会忘掉待发改动。 */
  trustRemote(confirm: () => Promise<boolean>): Promise<void>;
  /** 「测试连接」：用还没保存的草稿，通过时 resolve，失败时以带 `code` 的错误 reject。 */
  probe(draft: DictionarySyncWebDavPreferences): Promise<void>;
}

export interface UseDictionarySyncOptions {
  client: Pick<SettingsClient, "dictionarySync">;
  /** 已保存（不是草稿）的网盘配置：变了就重新以 `opened` 同步一次。 */
  saved: DictionarySyncWebDavPreferences | undefined;
  localChanges: DictionaryLocalChanges;
}

function savedKey(saved: DictionarySyncWebDavPreferences | undefined): string {
  return saved?.url ? `${saved.url}\n${saved.username}\n${saved.password}` : "";
}

/**
 * 设置打开期间的词库同步。配置了网盘时：挂载（或者保存了新的网盘配置）时以 `opened` 同步一次，之后按 Rust 给的 `next_check_ms` 以 `periodic` 再调；经设置改了词以 `local_change` 记一次（不联网，由 Rust 防抖）。要不要联网、额度够不够都由 Rust 决定，这里不算防抖和退避。
 *
 * 所有请求排成一队依次发出：Rust 那边整次同步持有一把文件锁，并发的请求只会互相等待。
 */
export function useDictionarySync({
  client,
  saved,
  localChanges,
}: UseDictionarySyncOptions): DictionarySyncModel | null {
  const sync = client.dictionarySync;
  const key = savedKey(saved);
  const configured = key !== "";
  const [status, setStatus] = useState<DictionarySyncStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [devices, setDevices] = useState<CloudClipboardDeviceStatusShape | null>(null);
  const [devicesLoading, setDevicesLoading] = useState(false);
  const [notice, setNotice] = useState("");
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  // 每换一次宿主或网盘配置加一：旧配置的请求回来时不再改动界面，也不再排下一次定时。
  const generation = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // 已经排在队里、还没开始的 `local_change`：连续改词只记一次。
  const localQueued = useRef(false);

  const enqueue = <T>(task: () => Promise<T>): Promise<T> => {
    const next = queue.current.then(task, task);
    queue.current = next.catch(() => undefined);
    return next;
  };

  const request = (action: DictionarySyncAction, owner = generation.current): Promise<unknown> => {
    if (!sync) return Promise.reject({ code: "dictionary_sync_not_configured" });
    return enqueue(() => {
      // 排队期间换过宿主或网盘时，旧设备管理操作不能发到新的网盘。
      if (owner !== generation.current)
        return Promise.reject({ code: "dictionary_sync_not_configured" });
      return sync.request(action);
    });
  };

  const schedule = (owner: number, nextCheckMs: number) => {
    if (owner !== generation.current) return;
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void run(owner, "periodic").catch(() => undefined);
    }, dictionarySyncDelay(nextCheckMs));
  };

  const applyRun = (owner: number, result: DictionarySyncRun | null) => {
    if (owner !== generation.current) return;
    if (!result) {
      setError("dictionary_sync_unavailable");
      return;
    }
    setStatus(result.status);
    setError(result.error);
    schedule(owner, result.next_check_ms);
  };

  async function run(
    owner: number,
    trigger: DictionarySyncTrigger,
    choice: { confirm_deletions?: boolean; restore_deletions?: boolean } = {},
  ): Promise<DictionarySyncRun | null> {
    if (owner !== generation.current) return null;
    const action: DictionarySyncAction = { operation: "run", trigger, ...choice };
    try {
      const value = await enqueue(async () => {
        if (owner !== generation.current) return null;
        if (trigger === "local_change") localQueued.current = false;
        setSyncing(true);
        try {
          return await sync!.request(action);
        } finally {
          if (owner === generation.current) setSyncing(false);
        }
      });
      if (value === null) return null;
      const result = dictionarySyncRun(value);
      applyRun(owner, result);
      return result;
    } catch (failure) {
      if (owner === generation.current) {
        setError(errorCode(failure) ?? "dictionary_sync_unavailable");
        // 没拿到下一次的时间（例如偏好读不出来）：半小时后再试，不在出错时连着重试。
        schedule(owner, 30 * 60 * 1000);
      }
      throw failure;
    }
  }

  useEffect(() => {
    generation.current += 1;
    const owner = generation.current;
    clearTimeout(timer.current);
    localQueued.current = false;
    setStatus(null);
    setError(null);
    setDevices(null);
    setDevicesLoading(false);
    setBusy(false);
    setSyncing(false);
    setNotice("");
    if (!sync || !configured) return;
    void run(owner, "opened").catch(() => undefined);
    const stop = localChanges.subscribe(() => {
      if (localQueued.current) return;
      localQueued.current = true;
      void run(owner, "local_change").catch(() => undefined);
    });
    // 宿主写进来的词走同一条通知：只在宿主提供了词库同步（鸿蒙要先有隐私政策的授权）并且配置了网盘时才听，连续几次同样只排一次。
    const hostChanged = () => localChanges.notify();
    if (typeof window !== "undefined") {
      window.addEventListener(HOST_USER_WORDS_CHANGED_EVENT, hostChanged);
    }
    return () => {
      stop();
      if (typeof window !== "undefined") {
        window.removeEventListener(HOST_USER_WORDS_CHANGED_EVENT, hostChanged);
      }
      generation.current += 1;
      clearTimeout(timer.current);
    };
    // 调度通过代次和引用保持状态，不随每次渲染重新启动。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sync, key, localChanges]);

  /** 用户发起的操作：期间显示忙，失败时记下错误码；配置换过之后回来的结果不再改动界面。 */
  const userAction = async (task: (owner: number) => Promise<void>): Promise<void> => {
    const owner = generation.current;
    setBusy(true);
    setNotice("");
    try {
      await task(owner);
    } catch (failure) {
      if (owner === generation.current)
        setError(errorCode(failure) ?? "dictionary_sync_unavailable");
    } finally {
      if (owner === generation.current) setBusy(false);
    }
  };

  const applyDevices = (owner: number, value: unknown) => {
    if (owner !== generation.current) return;
    const parsed = dictionarySyncDevices(value);
    if (parsed) setDevices(parsed);
  };

  return useMemo<DictionarySyncModel | null>(() => {
    if (!sync) return null;
    return {
      configured,
      status,
      error,
      busy,
      syncing,
      devices,
      devicesLoading,
      notice,
      syncNow: () =>
        userAction(async (owner) => {
          const result = await run(owner, "manual");
          if (result && !result.error && owner === generation.current) {
            setNotice(result.status.approved ? "已同步。" : "");
          }
        }),
      resolveDeletions: (choice) =>
        userAction(async (owner) => {
          await run(
            owner,
            "manual",
            choice === "confirm" ? { confirm_deletions: true } : { restore_deletions: true },
          );
        }),
      loadDevices: async (quiet = false) => {
        const load = async (owner: number) => {
          setDevicesLoading(true);
          try {
            applyDevices(owner, await request({ operation: "devices" }, owner));
            // 第一次取设备会在网盘上登记本机，核对码随后才有，所以再读一次不联网的概况。
            const next = dictionarySyncStatus(await request({ operation: "status" }, owner));
            if (next && owner === generation.current) setStatus(next);
          } finally {
            if (owner === generation.current) setDevicesLoading(false);
          }
        };
        if (!quiet) return userAction(load);
        await load(generation.current).catch(() => undefined);
      },
      approve: (deviceId) =>
        userAction(async (owner) => {
          applyDevices(owner, await request({ operation: "approve", device_id: deviceId }, owner));
          if (owner === generation.current) setError(null);
        }),
      reject: (deviceId) =>
        userAction(async (owner) => {
          applyDevices(owner, await request({ operation: "reject", device_id: deviceId }, owner));
          if (owner === generation.current) setError(null);
        }),
      remove: (deviceId) =>
        userAction(async (owner) => {
          const value = (await request({ operation: "remove", device_id: deviceId }, owner)) as {
            devices?: unknown;
            run?: unknown;
          } | null;
          applyDevices(owner, value?.devices);
          applyRun(owner, dictionarySyncRun(value?.run));
        }),
      reset: () =>
        userAction(async (owner) => {
          const value = (await request({ operation: "reset" }, owner)) as {
            devices?: unknown;
            run?: unknown;
          } | null;
          applyDevices(owner, value?.devices);
          applyRun(owner, dictionarySyncRun(value?.run));
        }),
      restart: () =>
        userAction(async (owner) => {
          const next = dictionarySyncStatus(await request({ operation: "restart" }, owner));
          if (owner !== generation.current) return;
          if (next) setStatus(next);
          setError(null);
          setNotice("已以本机的词条重新开始，其他设备获准后会把它们的词合并进来。");
        }),
      trustRemote: async (confirm) => {
        const owner = generation.current;
        if (!saved || !(await confirm()) || owner !== generation.current) return;
        await userAction(async () => {
          await request({ operation: "probe", ...saved, trust_remote: true }, owner);
          await run(owner, "manual");
        });
      },
      probe: async (draft) => {
        await request({ operation: "probe", ...draft });
      },
    };
    // 操作共享引用和请求队列，只在可见状态变化时重建模型。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sync, key, status, error, busy, syncing, devices, devicesLoading, notice]);
}
