import { useEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";

/** One catalog model as the host reports it, the shape of `LocalModelStatus` in `msime_client_core::voice::local_models`. */
export type LocalVoiceModel = {
  id: string;
  title: string;
  description: string;
  languages: string[];
  streaming: boolean;
  default: boolean;
  /** Too heavy for a phone; mobile hosts do not offer it. */
  desktop_only: boolean;
  installed: boolean;
  /** Where the model is (or would be) installed; what `asr_model_path` is set to when the user picks it. */
  path: string;
  installed_size: number;
  archive_size: number;
  /** Approximate resident memory while recognising, in bytes. */
  memory: number;
  license_spdx: string;
  license_source: string;
  license_terms: string;
  license_notice: string;
  /** `native` or `pinyin`: how the user's dictionary words reach the recognizer. */
  hotwords: string;
};
export type LocalVoiceModelList = { models: LocalVoiceModel[]; default: string; root: string };
export type LocalVoiceModelProgress = {
  id: string;
  stage: "download" | "verify" | "extract" | "done" | string;
  downloaded: number;
  total: number;
};
/** The host's on-device model store. Downloads use the saved `asr_model_mirror`. */
export type LocalVoiceModelClient = {
  list(): Promise<LocalVoiceModelList>;
  /** Resolves to the installed model directory once every file is verified and in place. */
  install(id: string): Promise<string>;
  cancel(id: string): Promise<boolean>;
  remove(id: string): Promise<void>;
  onProgress(listener: (progress: LocalVoiceModelProgress) => void): Promise<() => void>;
};

const LANGUAGE_NAMES: Record<string, string> = {
  zh: "中文",
  en: "英语",
  ja: "日语",
  ko: "韩语",
  yue: "粤语",
};

/** `1_234_567_890` -> `1.2 GB`; decimal units, the way the catalog and download sizes are quoted. */
export function formatModelBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "未知";
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
  return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
}

export function localModelLanguages(languages: readonly string[]): string {
  return languages.map((language) => LANGUAGE_NAMES[language] ?? language).join("、");
}

/** What this host can run: a phone does not offer the desktop-only models, unless one is already the model in use, which stays listed so it can be removed. */
export function visibleLocalModels(
  models: readonly LocalVoiceModel[],
  mobile: boolean,
  modelPath: string,
): LocalVoiceModel[] {
  return models.filter(
    (model) => !mobile || !model.desktop_only || localModelInUse(model, modelPath),
  );
}

/** Whether `modelPath` names this model's directory, ignoring surrounding space and a trailing separator. */
export function localModelInUse(model: LocalVoiceModel, modelPath: string): boolean {
  const normalize = (path: string) => path.trim().replace(/[\\/]+$/, "");
  const path = normalize(modelPath);
  return path !== "" && path === normalize(model.path);
}

/** Whole-percent progress of one install; the verify and extract stages report the archive bytes they have consumed. */
export function localModelProgressPercent(progress: LocalVoiceModelProgress | undefined): number {
  if (!progress || progress.total <= 0) return 0;
  if (progress.stage === "done") return 100;
  return Math.min(100, Math.max(0, Math.floor((progress.downloaded / progress.total) * 100)));
}

export function localModelStageLabel(stage: string): string {
  switch (stage) {
    case "download":
      return "下载中";
    case "verify":
      return "校验中";
    case "extract":
      return "解压中";
    case "done":
      return "完成";
    default:
      return "准备中";
  }
}

/** The message for a failed list, install or removal; `null` for a cancel the user asked for. */
export function localModelErrorMessage(error: unknown): string | null {
  switch (errorCode(error)) {
    case "local_model_cancelled":
      return null;
    case "local_model_network":
      return "下载失败：无法连接下载服务器。请检查网络，或在下方填写下载镜像后保存设置再试。";
    case "local_model_http_status":
      return "下载失败：服务器拒绝了请求。请稍后重试，或更换下载镜像。";
    case "local_model_checksum_mismatch":
      return "下载的文件校验不通过，已丢弃。请重试；若使用了镜像，请确认镜像内容完整。";
    case "local_model_invalid_archive":
      return "模型文件内容不完整或不安全，已丢弃。请重试。";
    case "local_model_invalid_mirror":
      return "下载镜像地址无效，必须以 https:// 开头。";
    case "local_model_io":
      return "无法写入模型文件，请确认磁盘空间充足。";
    case "busy":
      return "该模型正在下载，请等待完成或先取消。";
    case "local_model_unknown":
      return "未知的模型。";
    case "local_model_invalid_root":
    case "unavailable":
      return "无法访问模型存放目录。";
    default:
      return "操作失败，请重试。";
  }
}

/** Whether a mirror field value would pass the host's validation: empty, or an `https://` prefix without spaces or control characters. */
export function validModelMirror(mirror: string): boolean {
  return (
    mirror === "" ||
    (mirror.length <= 2048 &&
      mirror.length > "https://".length &&
      mirror.startsWith("https://") &&
      // eslint-disable-next-line no-control-regex
      !/[\s\u0000-\u001f\u007f]/.test(mirror))
  );
}

/**
 * The on-device models the `local` provider can run: what each is, what it costs, and download, use and remove.
 *
 * "Use" only edits the page's draft (`asr_model_path`); it takes effect when the settings are saved, like every other field on the page.
 */
export function LocalModelManager({
  client,
  mobile,
  modelPath,
  onUse,
  onRemoved,
  confirm,
  openExternalUrl,
}: {
  client: LocalVoiceModelClient;
  mobile: boolean;
  modelPath: string;
  onUse: (path: string) => void;
  onRemoved: (model: LocalVoiceModel) => void;
  confirm: (request: {
    message: string;
    title?: string;
    confirmLabel?: string;
    danger?: boolean;
  }) => Promise<boolean>;
  openExternalUrl?: (url: string) => Promise<void>;
}) {
  const [list, setList] = useState<LocalVoiceModelList>();
  const [notice, setNotice] = useState("");
  const [progress, setProgress] = useState<Record<string, LocalVoiceModelProgress>>({});
  const [installing, setInstalling] = useState<Record<string, boolean>>({});
  const [removing, setRemoving] = useState<Record<string, boolean>>({});
  const mounted = useRef(true);
  const activeClient = useRef(client);
  activeClient.current = client;
  // A download takes minutes; what it finishes into is the page as it is then, not as it was on
  // the click that started it.
  const modelPathRef = useRef(modelPath);
  modelPathRef.current = modelPath;
  const onUseRef = useRef(onUse);
  onUseRef.current = onUse;

  const refresh = async () => {
    try {
      const next = await client.list();
      if (mounted.current && activeClient.current === client) setList(next);
    } catch (error) {
      if (mounted.current && activeClient.current === client)
        setNotice(localModelErrorMessage(error) ?? "");
    }
  };

  useEffect(() => {
    mounted.current = true;
    void refresh();
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void client
      .onProgress((event) => {
        if (mounted.current && activeClient.current === client)
          setProgress((current) => ({ ...current, [event.id]: event }));
      })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      mounted.current = false;
      cancelled = true;
      unlisten?.();
    };
  }, [client]);

  const install = async (model: LocalVoiceModel) => {
    setNotice("");
    setInstalling((current) => ({ ...current, [model.id]: true }));
    setProgress((current) => ({
      ...current,
      [model.id]: { id: model.id, stage: "download", downloaded: 0, total: model.archive_size },
    }));
    try {
      const path = await client.install(model.id);
      if (!mounted.current || activeClient.current !== client) return;
      setNotice(`「${model.title}」已下载。`);
      // A first model is what the user downloaded it for; a later one waits to be picked.
      if (!modelPathRef.current.trim()) onUseRef.current(path);
    } catch (error) {
      if (mounted.current && activeClient.current === client)
        setNotice(localModelErrorMessage(error) ?? `已取消下载「${model.title}」。`);
    } finally {
      if (mounted.current && activeClient.current === client) {
        setInstalling((current) => ({ ...current, [model.id]: false }));
        setProgress((current) => {
          const next = { ...current };
          delete next[model.id];
          return next;
        });
        await refresh();
      }
    }
  };

  const remove = async (model: LocalVoiceModel) => {
    const confirmed = await confirm({
      title: "删除本地模型",
      message: localModelInUse(model, modelPath)
        ? `「${model.title}」正在使用。删除后本地识别将不可用，直到选择其他模型。确定删除？`
        : `确定删除「${model.title}」？需要时可以重新下载。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed) return;
    setNotice("");
    setRemoving((current) => ({ ...current, [model.id]: true }));
    try {
      await client.remove(model.id);
      if (!mounted.current || activeClient.current !== client) return;
      onRemoved(model);
    } catch (error) {
      if (mounted.current && activeClient.current === client)
        setNotice(localModelErrorMessage(error) ?? "");
    } finally {
      if (mounted.current && activeClient.current === client) {
        setRemoving((current) => ({ ...current, [model.id]: false }));
        await refresh();
      }
    }
  };

  const models = list ? visibleLocalModels(list.models, mobile, modelPath) : [];
  return (
    <div className="section" aria-label="本地识别模型">
      <div className="section-title">
        本地识别模型
        <small>模型下载到本机后完全离线运行，录音不会上传。点击“使用”后保存设置生效。</small>
      </div>
      {!list && !notice && <p>正在读取模型列表…</p>}
      <ul className="grid gap-3" aria-label="可用的本地模型">
        {models.map((model) => {
          const inUse = localModelInUse(model, modelPath);
          const running = installing[model.id] === true;
          const current = progress[model.id];
          const percent = localModelProgressPercent(current);
          return (
            <li
              key={model.id}
              aria-label={model.title}
              className="grid gap-1 rounded-lg border border-[var(--border,#d0d0d0)] p-3"
            >
              <div className="flex flex-wrap items-center gap-2">
                <strong>{model.title}</strong>
                {model.streaming && <span className="text-xs opacity-70">流式</span>}
                {model.default && <span className="text-xs opacity-70">推荐</span>}
                {inUse && <span className="text-xs font-semibold">正在使用</span>}
                {model.installed && !inUse && <span className="text-xs opacity-70">已下载</span>}
              </div>
              <p>{model.description}</p>
              <p className="text-xs opacity-70">
                语言：{localModelLanguages(model.languages)} · 下载{" "}
                {formatModelBytes(model.archive_size)} · 占用磁盘{" "}
                {formatModelBytes(model.installed_size)} · 运行内存约{" "}
                {formatModelBytes(model.memory)}
              </p>
              <p className="text-xs opacity-70">
                许可：{model.license_spdx}。{model.license_notice} 来源：
                {openExternalUrl ? (
                  <button
                    type="button"
                    className="link"
                    onClick={() =>
                      void openExternalUrl(model.license_source).catch(() => undefined)
                    }
                  >
                    {model.license_source}
                  </button>
                ) : (
                  <span>{model.license_source}</span>
                )}
                {model.license_terms && <span>（条款：{model.license_terms}）</span>}
              </p>
              {running && (
                <div className="flex items-center gap-2">
                  <progress
                    aria-label={`${model.title} 下载进度`}
                    max={100}
                    value={percent}
                    className="min-w-0 flex-1"
                  />
                  <span className="text-xs">
                    {localModelStageLabel(current?.stage ?? "")} {percent}%
                  </span>
                </div>
              )}
              <div className="flex flex-wrap gap-2">
                {running ? (
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => void client.cancel(model.id).catch(() => undefined)}
                  >
                    取消下载
                  </button>
                ) : model.installed ? (
                  <>
                    <button
                      type="button"
                      disabled={inUse || removing[model.id] === true}
                      onClick={() => onUse(model.path)}
                    >
                      {inUse ? "使用中" : "使用"}
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={removing[model.id] === true}
                      onClick={() => void remove(model)}
                    >
                      删除
                    </button>
                  </>
                ) : (
                  <button type="button" onClick={() => void install(model)}>
                    下载（{formatModelBytes(model.archive_size)}）
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      {notice && <p role="status">{notice}</p>}
    </div>
  );
}
