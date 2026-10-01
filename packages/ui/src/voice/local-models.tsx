import { useEffect, useRef, useState } from "react";
import * as settings from "../settings/settings-style";
import { rowTitle } from "../core/platform-controls-style";
import {
  formatModelBytes,
  localModelErrorMessage,
  localModelInUse,
  localModelLanguages,
  localModelProgressPercent,
  localModelStageLabel,
  visibleLocalModels,
} from "./local-model-helpers";
export {
  formatModelBytes,
  localModelErrorMessage,
  localModelInUse,
  localModelLanguages,
  localModelProgressPercent,
  localModelStageLabel,
  validModelMirror,
  visibleLocalModels,
} from "./local-model-helpers";

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

const localModelNote = "模型下载到本机后完全离线运行，录音不会上传。点击“使用”后生效。";

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
  grouped = false,
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
  /** 放在语音页「识别服务配置」组里时为真：画成组内的一块，标题和说明用组内的字号，不再自带卡片。 */
  grouped?: boolean;
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
    <div
      className={grouped ? settings.managerBlock : "section"}
      role={grouped ? "group" : undefined}
      aria-label="本地识别模型"
    >
      {grouped ? (
        <div>
          <span className={rowTitle} data-row-title="">
            本地识别模型
          </span>
          <p className={settings.managerNote}>{localModelNote}</p>
        </div>
      ) : (
        <div className="section-title">
          本地识别模型
          <small>{localModelNote}</small>
        </div>
      )}
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
