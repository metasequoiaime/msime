import { useCallback, useEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";
import { useAsyncGeneration } from "./use-async-generation";
import { useMountedRef } from "./use-mounted-ref";
import { ActionRow } from "./action-row";
import { VoiceModelMirrorRow } from "./voice-model-mirror-section";
import { defaultVoiceInput } from "./voice-input-defaults";
import type { SettingsClient } from "../index";
import type { LocalVoiceModelProgress } from "../voice/local-models";
import {
  formatModelBytes,
  localModelErrorMessage,
  localModelProgressPercent,
  localModelStageLabel,
} from "../voice/local-model-helpers";

/** 按需下载的资源包 id，与 `msime_client_core::resource_packs::ResourcePack::id` 一致。 */
export type ResourcePackId = "japanese" | "language-dictionaries" | "handwriting" | "settled-model";

/** 宿主报告的资源包状态，即 `msime_client_core::resource_packs::ResourcePackStatus`。`outdated` 表示已下载的文件字节（名字、SHA-256、长度）与当前锁文件不一致：输入法不再使用这份旧文件，要重新下载。只换了下载地址、字节没变的资源包仍是 `installed`。 */
export type ResourcePackStatus = {
  id: ResourcePackId;
  state: "missing" | "installed" | "outdated";
  /** 下载大小，字节。 */
  size: number;
  /** 需要这个资源包的输入方案。 */
  schemes: string[];
};

/** 宿主的资源包下载服务，三个桌面宿主提供。列表里只有本机需要下载的资源包（例如 Windows 上 Ink 有中文识别器时不列手写模型），没列出的就不必下载。镜像沿用已保存的 `voice_input.asr_model_mirror`。 */
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
  "settled-model": "桌面神经联想模型",
};

/** 资源包和本地语音模型共用的下载镜像（`voice_input.asr_model_mirror`）在设置页里的草稿值和修改入口；下载失败时资源包行就地提供它。 */
export type ResourcePackMirror = {
  value: string;
  onChange(value: string): void;
  /** 立即写入还在自动保存倒计时里的编辑。下载读的是已保存的镜像，所以失败行的「重试」先调它，刚填的镜像才会用上。 */
  flush?(): Promise<void>;
};

/** 已保存的模型下载镜像（`voice_input.asr_model_mirror`），给不在设置页里的界面（手写面板）用。`save` 写入偏好，之后的下载才会用它。 */
export type ModelMirrorClient = {
  load(): Promise<string>;
  save(mirror: string): Promise<void>;
};

/** 经设置服务读写已保存的下载镜像：每次保存前重新读一遍偏好，只改镜像这一项。 */
export function savedModelMirror(client: Pick<SettingsClient, "load" | "save">): ModelMirrorClient {
  return {
    load: async () => (await client.load()).preferences.voice_input?.asr_model_mirror ?? "",
    save: async (mirror) => {
      const snapshot = await client.load();
      await client.save(snapshot.revision, {
        ...snapshot.preferences,
        voice_input: {
          ...defaultVoiceInput,
          ...snapshot.preferences.voice_input,
          asr_model_mirror: mirror,
        },
      });
    },
  };
}

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
  /** 资源包列出、还没装好、也没有在下载时开始下载。列表还没读到时先记下，读到后再按同样的条件下载。 */
  ensure(id: ResourcePackId): void;
  install(id: ResourcePackId): void;
  cancel(id: ResourcePackId): void;
  /** 下载失败时就地设置下载镜像用；不提供时失败行只有原因和重试。 */
  mirror?: ResourcePackMirror;
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
export function useResourcePacks(
  client?: ResourcePackClient,
  mirror?: ResourcePackMirror,
): ResourcePacks {
  const [statuses, setStatuses] = useState<ResourcePackStatus[]>();
  const [progress, setProgress] = useState<ResourcePacks["progress"]>({});
  const [errors, setErrors] = useState<ResourcePacks["errors"]>({});
  const mounted = useMountedRef();
  const clientGeneration = useAsyncGeneration(client);
  const activeClient = useRef(client);
  activeClient.current = client;
  const statusesRef = useRef(statuses);
  statusesRef.current = statuses;
  // 本页发起、尚未结束的下载；进度事件可能晚到，所以不能只看 progress 判断。
  const running = useRef(new Set<string>());
  const runningKey = (generation: number, id: ResourcePackId) => `${generation}:${id}`;
  // 列表读到之前请求的 ensure：读到列表后再判断要不要下载，免得开关打开得早、下载就悄悄没了。
  const pendingEnsure = useRef(new Set<ResourcePackId>());

  const current = (expected: ResourcePackClient, generation: number) =>
    mounted.current && activeClient.current === expected && clientGeneration.current === generation;

  const refresh = useCallback(async (expected: ResourcePackClient, generation: number) => {
    try {
      const next = await expected.list();
      if (current(expected, generation)) setStatuses(next);
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
    const generation = clientGeneration.current;
    running.current = new Set();
    pendingEnsure.current = new Set();
    setStatuses(undefined);
    setProgress({});
    setErrors({});
    if (!client) return;
    void refresh(client, generation);
    let unlisten: (() => void) | undefined;
    void client
      .onProgress((event) => {
        if (!current(client, generation)) return;
        const id = event.id as ResourcePackId;
        // 启动时自动补齐的下载也会发进度；它结束时页面靠 done 事件重新读列表。
        if (event.stage === "done" && !running.current.has(runningKey(generation, id))) {
          clearProgress(id);
          void refresh(client, generation);
          return;
        }
        setProgress((existing) => ({ ...existing, [id]: event }));
      })
      .then((stop) => {
        if (generation !== clientGeneration.current) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      unlisten?.();
    };
  }, [client, clientGeneration, refresh]);

  const install = (id: ResourcePackId) => {
    const expected = client;
    const generation = clientGeneration.current;
    const key = runningKey(generation, id);
    if (!expected || !current(expected, generation) || running.current.has(key)) return;
    running.current.add(key);
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
        if (current(expected, generation) && errorCode(error) !== "busy")
          setErrors((existing) => ({
            ...existing,
            [id]: localModelErrorMessage(error) ?? undefined,
          }));
      } finally {
        running.current.delete(key);
        if (current(expected, generation)) {
          clearProgress(id);
          await refresh(expected, generation);
        }
      }
    })();
  };

  const ensure = (id: ResourcePackId) => {
    const generation = clientGeneration.current;
    if (!client || !current(client, generation)) return;
    if (!statusesRef.current) {
      pendingEnsure.current.add(id);
      return;
    }
    const status = resourcePackStatus({ statuses: statusesRef.current }, id);
    if (!status || status.state === "installed" || running.current.has(runningKey(generation, id))) return;
    install(id);
  };

  useEffect(() => {
    if (!statuses || pendingEnsure.current.size === 0) return;
    const pending = [...pendingEnsure.current];
    pendingEnsure.current = new Set();
    pending.forEach(ensure);
  }, [statuses]);

  const cancel = (id: ResourcePackId) => {
    const expected = client;
    const generation = clientGeneration.current;
    if (!expected) return;
    void expected
      .cancel(id)
      .then((stopped) => {
        // 宿主那边已经没有安装在跑（例如启动时的自动补齐中途失败），清掉残留的进度。
        if (!stopped && current(expected, generation) && !running.current.has(runningKey(generation, id))) clearProgress(id);
      })
      .catch(() => undefined);
  };

  return { statuses, progress, errors, ensure, install, cancel, mirror };
}

/** 资源包未安装时的一行：说明大小并提供下载；下载中显示进度和取消；失败时显示原因和重试，并提供设置下载镜像的入口。已安装或宿主不提供下载服务时不渲染。 */
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
      <>
        <ActionRow
          title={title}
          description={error}
          action={async () => {
            // 宿主按已保存的偏好取镜像：刚在下面填的镜像可能还在自动保存的倒计时里，先写进去再下载。
            await packs.mirror?.flush?.();
            packs.install(id);
          }}
          label="重试"
          ariaLabel={`重新下载${title}`}
        />
        {packs.mirror && <ResourcePackMirrorEntry mirror={packs.mirror} />}
      </>
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

/** 下载失败后的镜像入口：先是一个「设置下载镜像」按钮，点开后就地显示镜像输入框，填好后点「重试」（重试先保存还在倒计时里的编辑）。已经填过镜像时直接显示输入框，方便核对。 */
function ResourcePackMirrorEntry({ mirror }: { mirror: ResourcePackMirror }) {
  const [open, setOpen] = useState(() => mirror.value.trim() !== "");
  if (open) return <VoiceModelMirrorRow value={mirror.value} onChange={mirror.onChange} />;
  return (
    <ActionRow
      title="下载镜像"
      description="无法连接 GitHub 时，可以填写一个镜像前缀，再重新下载"
      action={() => setOpen(true)}
      label="设置下载镜像"
    />
  );
}
