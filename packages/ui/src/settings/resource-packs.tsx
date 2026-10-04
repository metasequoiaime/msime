import { useCallback, useEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";
import { useMountedRef } from "./use-mounted-ref";
import { ActionRow } from "./action-row";
import type { LocalVoiceModelProgress } from "../voice/local-models";
import {
  formatModelBytes,
  localModelErrorMessage,
  localModelProgressPercent,
  localModelStageLabel,
} from "../voice/local-model-helpers";

/** 按需下载的资源包 id，与 `msime_client_core::resource_packs::ResourcePack::id` 一致。 */
export type ResourcePackId = "japanese" | "language-dictionaries" | "handwriting";

/** 宿主报告的资源包状态，即 `msime_client_core::resource_packs::ResourcePackStatus`。`outdated` 表示已下载但与当前锁文件不一致，旧文件仍可使用。 */
export type ResourcePackStatus = {
  id: ResourcePackId;
  state: "missing" | "installed" | "outdated";
  /** 下载大小，字节。 */
  size: number;
  /** 需要这个资源包的输入方案。 */
  schemes: string[];
};

/** 宿主的资源包下载服务。目前只有 macOS 提供；镜像沿用已保存的 `voice_input.asr_model_mirror`。 */
export type ResourcePackClient = {
  list(): Promise<ResourcePackStatus[]>;
  /** 下载、校验并发布完成后返回安装目录。 */
  install(id: ResourcePackId): Promise<string>;
  /** 返回是否有安装在跑；被取消的安装随后以 `local_model_cancelled` 失败。 */
  cancel(id: ResourcePackId): Promise<boolean>;
  onProgress(listener: (progress: LocalVoiceModelProgress) => void): Promise<() => void>;
};

export const resourcePackTitles: Record<ResourcePackId, string> = {
  japanese: "日文词库",
  "language-dictionaries": "粤语、注音与笔画词库",
  handwriting: "手写模型",
};

/** 选用某个输入方案时需要下载的资源包；不需要额外资源的方案返回 undefined。 */
export function resourcePackForScheme(scheme: string | undefined): ResourcePackId | undefined {
  if (scheme === "japanese") return "japanese";
  if (scheme === "cantonese" || scheme === "zhuyin" || scheme === "stroke")
    return "language-dictionaries";
  return undefined;
}

export type ResourcePacks = {
  /** 还没读到列表（或宿主不提供下载服务）时为 undefined。 */
  statuses?: ResourcePackStatus[];
  /** 正在下载的资源包及其进度；有条目即表示在下载。 */
  progress: Partial<Record<ResourcePackId, LocalVoiceModelProgress>>;
  errors: Partial<Record<ResourcePackId, string>>;
  /** 列表已读到、资源包还没装好、也没有在下载时才开始下载。 */
  ensure(id: ResourcePackId): void;
  install(id: ResourcePackId): void;
  cancel(id: ResourcePackId): void;
};

/** 查某个资源包的状态。 */
export function resourcePackStatus(
  packs: Pick<ResourcePacks, "statuses">,
  id: ResourcePackId,
): ResourcePackStatus | undefined {
  return packs.statuses?.find((status) => status.id === id);
}

/**
 * 读取并下载按需资源包。`client` 为 undefined 时什么都不做，statuses 保持 undefined。
 *
 * 失败只记录错误、等用户点「重试」，这里不会自动重试。
 */
export function useResourcePacks(client?: ResourcePackClient): ResourcePacks {
  const [statuses, setStatuses] = useState<ResourcePackStatus[]>();
  const [progress, setProgress] = useState<ResourcePacks["progress"]>({});
  const [errors, setErrors] = useState<ResourcePacks["errors"]>({});
  const mounted = useMountedRef();
  const activeClient = useRef(client);
  activeClient.current = client;
  const statusesRef = useRef(statuses);
  statusesRef.current = statuses;
  // 本页发起、尚未结束的下载；进度事件可能晚到，所以不能只看 progress 判断。
  const running = useRef(new Set<ResourcePackId>());

  const current = (expected: ResourcePackClient) =>
    mounted.current && activeClient.current === expected;

  const refresh = useCallback(async (expected: ResourcePackClient) => {
    try {
      const next = await expected.list();
      if (mounted.current && activeClient.current === expected) setStatuses(next);
    } catch {
      // 读不到列表时不提供下载入口，输入法照常按缺少资源降级。
    }
  }, []);

  const clearProgress = (id: ResourcePackId) =>
    setProgress((existing) => {
      if (!(id in existing)) return existing;
      const next = { ...existing };
      delete next[id];
      return next;
    });

  useEffect(() => {
    running.current = new Set();
    setStatuses(undefined);
    setProgress({});
    setErrors({});
    if (!client) return;
    void refresh(client);
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void client
      .onProgress((event) => {
        if (!current(client)) return;
        const id = event.id as ResourcePackId;
        // 启动时自动补齐的下载也会发进度；它结束时页面靠 done 事件重新读列表。
        if (event.stage === "done" && !running.current.has(id)) {
          clearProgress(id);
          void refresh(client);
          return;
        }
        setProgress((existing) => ({ ...existing, [id]: event }));
      })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [client, refresh]);

  const install = (id: ResourcePackId) => {
    const expected = client;
    if (!expected || running.current.has(id)) return;
    running.current.add(id);
    const size = resourcePackStatus({ statuses: statusesRef.current }, id)?.size ?? 0;
    setErrors((existing) => ({ ...existing, [id]: undefined }));
    setProgress((existing) => ({
      ...existing,
      [id]: { id, stage: "download", downloaded: 0, total: size },
    }));
    void (async () => {
      try {
        await expected.install(id);
      } catch (error) {
        // busy：启动时的自动补齐已经在下载这个包，进度事件会接着显示，不算失败。
        if (current(expected) && errorCode(error) !== "busy")
          setErrors((existing) => ({
            ...existing,
            [id]: localModelErrorMessage(error) ?? undefined,
          }));
      } finally {
        running.current.delete(id);
        if (current(expected)) {
          clearProgress(id);
          await refresh(expected);
        }
      }
    })();
  };

  const ensure = (id: ResourcePackId) => {
    const status = resourcePackStatus({ statuses: statusesRef.current }, id);
    if (!status || status.state === "installed" || running.current.has(id)) return;
    install(id);
  };

  const cancel = (id: ResourcePackId) => {
    const expected = client;
    if (!expected) return;
    void expected
      .cancel(id)
      .then((stopped) => {
        // 宿主那边已经没有安装在跑（例如启动时的自动补齐中途失败），清掉残留的进度。
        if (!stopped && current(expected) && !running.current.has(id)) clearProgress(id);
      })
      .catch(() => undefined);
  };

  return { statuses, progress, errors, ensure, install, cancel };
}

/** 资源包未安装时的一行：说明大小并提供下载；下载中显示进度和取消；失败时显示原因和重试。已安装或宿主不提供下载服务时不渲染。 */
export function ResourcePackRow({
  packs,
  id,
  note,
}: {
  packs: ResourcePacks;
  id: ResourcePackId;
  note?: string;
}) {
  const status = resourcePackStatus(packs, id);
  if (!status || status.state === "installed") return null;
  const title = resourcePackTitles[id];
  const progress = packs.progress[id];
  if (progress) {
    return (
      <ActionRow
        title={title}
        description={`${localModelStageLabel(progress.stage)} ${localModelProgressPercent(progress)}%`}
        action={() => packs.cancel(id)}
        label="取消"
        ariaLabel={`取消下载${title}`}
        ariaBusy
      />
    );
  }
  const error = packs.errors[id];
  if (error) {
    return (
      <ActionRow
        title={title}
        description={error}
        action={() => packs.install(id)}
        label="重试"
        ariaLabel={`重新下载${title}`}
      />
    );
  }
  return (
    <ActionRow
      title={title}
      description={`约 ${formatModelBytes(status.size)}，下载后切换一次输入框即可使用${note ? `。${note}` : ""}`}
      action={() => packs.install(id)}
      label="下载"
      ariaLabel={`下载${title}`}
    />
  );
}
