import { useEffect, useRef, useState } from "react";
import { useLatestRef } from "../core/use-latest-ref";
import { SettingsManagerBlock } from "../settings/settings-manager-block";
import { InfoTip } from "../core/info-tip";
import { rowTitle } from "../core/platform-controls-style";
import { ActionButton } from "../core/action-button";
import {
  formatModelBytes,
  localModelErrorMessage,
  localModelImportErrorMessage,
  localModelInUse,
  localModelLanguages,
  localModelProgressPercent,
  localModelStageLabel,
  visibleLocalModels,
} from "./local-model-helpers";
import { StatusMessage } from "../core/status-message";
import { trackAsyncSubscription } from "../core/async-subscription";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useMountedRef } from "../settings/use-mounted-ref";
export {
  formatModelBytes,
  localModelErrorMessage,
  localModelImportErrorMessage,
  localModelInUse,
  localModelLanguages,
  localModelProgressPercent,
  localModelStageLabel,
  validModelMirror,
  visibleLocalModels,
} from "./local-model-helpers";

/** 宿主报告的一个目录模型，形状与 `msime_client_providers::voice::local_models` 里的 `LocalModelStatus` 相同。 */
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
  /** 不联网安装时要用户自己下载的文件：模型压缩包在前，带下载地址的附加文件在后。 */
  import_files: LocalVoiceModelImportFile[];
};
/** 「从文件导入」需要的一个文件，`url` 是它在上游的下载地址。 */
export type LocalVoiceModelImportFile = { name: string; url: string; size: number };
export type LocalVoiceModelList = { models: LocalVoiceModel[]; default: string; root: string };
export type LocalVoiceModelProgress = {
  id: string;
  stage: "download" | "import" | "verify" | "extract" | "done" | string;
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
  /**
   * 让用户用宿主的文件选择器选自己下载好的文件来安装 `id`，不联网；安装好后解析为模型目录，用户关掉选择器时为 `null`。进度和下载一样经 `onProgress` 报告，阶段是 `import`。宿主不提供时页面不显示导入按钮。
   */
  import?(id: string): Promise<string | null>;
  onProgress(listener: (progress: LocalVoiceModelProgress) => void): Promise<() => void>;
};

/** 许可来源和不联网安装文件的下载地址：行内的文字链接，不画成按钮；长网址和文件名随卡片 `overflow-wrap:anywhere` 断行，不把卡片撑出屏幕。 */
const inlineLink =
  "m-0 inline cursor-pointer border-0 bg-transparent p-0 text-left text-xs [font-family:inherit] [color:var(--p-accent-text,var(--accent-color))] underline-offset-2 hover:underline active:underline";

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
  const [importing, setImporting] = useState<Record<string, boolean>>({});
  const mounted = useMountedRef();
  const clientGeneration = useAsyncGeneration(client);
  const activeClient = useLatestRef(client);
  const refreshRequest = useRef(0);
  // A download takes minutes; what it finishes into is the page as it is then, not as it was on
  // the click that started it.
  const modelPathRef = useLatestRef(modelPath);
  const onUseRef = useLatestRef(onUse);

  const current = (generation: number) =>
    mounted.current && activeClient.current === client && clientGeneration.current === generation;

  const refresh = async (generation: number) => {
    if (!current(generation)) return;
    const request = ++refreshRequest.current;
    try {
      const next = await client.list();
      if (current(generation) && request === refreshRequest.current) setList(next);
    } catch (error) {
      if (current(generation) && request === refreshRequest.current)
        setNotice(localModelErrorMessage(error) ?? "");
    }
  };

  useEffect(() => {
    const generation = clientGeneration.current;
    setList(undefined);
    setNotice("");
    setProgress({});
    setInstalling({});
    setRemoving({});
    setImporting({});
    void refresh(generation);
    return trackAsyncSubscription(
      client.onProgress((event) => {
        if (current(generation)) setProgress((current) => ({ ...current, [event.id]: event }));
      }),
      () => generation === clientGeneration.current,
    );
  }, [client, clientGeneration]);

  const install = async (model: LocalVoiceModel) => {
    const generation = clientGeneration.current;
    if (!current(generation)) return;
    setNotice("");
    setInstalling((current) => ({ ...current, [model.id]: true }));
    setProgress((current) => ({
      ...current,
      [model.id]: { id: model.id, stage: "download", downloaded: 0, total: model.archive_size },
    }));
    try {
      const path = await client.install(model.id);
      if (!current(generation)) return;
      setNotice(`「${model.title}」已下载。`);
      // A first model is what the user downloaded it for; a later one waits to be picked.
      if (!modelPathRef.current.trim()) onUseRef.current(path);
    } catch (error) {
      if (current(generation))
        setNotice(localModelErrorMessage(error) ?? `已取消下载「${model.title}」。`);
    } finally {
      if (current(generation)) {
        setInstalling((current) => ({ ...current, [model.id]: false }));
        setProgress((current) => {
          const next = { ...current };
          delete next[model.id];
          return next;
        });
        await refresh(generation);
      }
    }
  };

  const importFiles = async (model: LocalVoiceModel) => {
    const generation = clientGeneration.current;
    if (!client.import || !current(generation)) return;
    setNotice("");
    setImporting((current) => ({ ...current, [model.id]: true }));
    try {
      const path = await client.import(model.id);
      if (!current(generation)) return;
      // 用户关掉了选择器，不算失败。
      if (path === null) return;
      setNotice(`「${model.title}」已导入。`);
      // 和下载一样：第一个装上的模型直接用上，之后的等用户选。
      if (!modelPathRef.current.trim()) onUseRef.current(path);
    } catch (error) {
      if (current(generation))
        setNotice(localModelImportErrorMessage(error) ?? `已取消导入「${model.title}」。`);
    } finally {
      if (current(generation)) {
        setImporting((current) => ({ ...current, [model.id]: false }));
        setProgress((current) => {
          const next = { ...current };
          delete next[model.id];
          return next;
        });
        await refresh(generation);
      }
    }
  };

  const remove = async (model: LocalVoiceModel) => {
    const generation = clientGeneration.current;
    if (!current(generation)) return;
    const confirmed = await confirm({
      title: "删除本地模型",
      message: localModelInUse(model, modelPath)
        ? `「${model.title}」正在使用。删除后本地识别将不可用，直到选择其他模型。确定删除？`
        : `确定删除「${model.title}」？需要时可以重新下载。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed || !current(generation)) return;
    setNotice("");
    setRemoving((current) => ({ ...current, [model.id]: true }));
    try {
      await client.remove(model.id);
      if (!current(generation)) return;
      onRemoved(model);
    } catch (error) {
      if (current(generation)) setNotice(localModelErrorMessage(error) ?? "");
    } finally {
      if (current(generation)) {
        setRemoving((current) => ({ ...current, [model.id]: false }));
        await refresh(generation);
      }
    }
  };

  const models = list ? visibleLocalModels(list.models, mobile, modelPath) : [];
  return (
    <SettingsManagerBlock role="group" aria-label="本地识别模型">
      <div className="flex items-center gap-1">
        <span className={rowTitle} data-row-title="">
          本地识别模型
        </span>
        <InfoTip subject="本地识别模型">{localModelNote}</InfoTip>
      </div>
      {!list && !notice && <p>正在读取模型列表…</p>}
      <ul className="m-0 grid list-none gap-3 p-0" aria-label="可用的本地模型">
        {models.map((model) => {
          const inUse = localModelInUse(model, modelPath);
          const running = installing[model.id] === true;
          const importRunning = importing[model.id] === true;
          const current = progress[model.id];
          const percent = localModelProgressPercent(current);
          return (
            <li
              key={model.id}
              aria-label={model.title}
              className="grid min-w-0 gap-1 rounded-lg border border-[var(--border,#d0d0d0)] p-3 [overflow-wrap:anywhere]"
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
                  <ActionButton
                    action={() => void openExternalUrl(model.license_source).catch(() => undefined)}
                    className={inlineLink}
                    label={model.license_source}
                  />
                ) : (
                  <span>{model.license_source}</span>
                )}
                {model.license_terms && <span>（条款：{model.license_terms}）</span>}
              </p>
              {!model.installed && client.import && model.import_files.length > 0 && (
                <p className="text-xs opacity-70">
                  不联网安装：先下载{" "}
                  {model.import_files.map((file, index) => (
                    <span key={file.url}>
                      {index > 0 && "、"}
                      {openExternalUrl ? (
                        <ActionButton
                          action={() => void openExternalUrl(file.url).catch(() => undefined)}
                          className={inlineLink}
                          label={`${file.name}（${formatModelBytes(file.size)}）`}
                        />
                      ) : (
                        <span>
                          {file.name}（{formatModelBytes(file.size)}）：{file.url}
                        </span>
                      )}
                    </span>
                  ))}
                  ，再点“从文件导入”一起选中。文件改过名也能认出。
                </p>
              )}
              {(running || (importRunning && current)) && (
                <div className="flex items-center gap-2">
                  <progress
                    aria-label={`${model.title} ${importRunning ? "导入" : "下载"}进度`}
                    max={100}
                    value={percent}
                    className="min-w-0 flex-1"
                  />
                  <span className="text-xs">
                    {localModelStageLabel(current?.stage ?? "")} {percent}%
                  </span>
                </div>
              )}
              <div className="flex flex-wrap items-center gap-2">
                {running || importRunning ? (
                  <ActionButton
                    action={() => void client.cancel(model.id).catch(() => undefined)}
                    className="secondary m-0"
                    label={importRunning ? "取消导入" : "取消下载"}
                  />
                ) : model.installed ? (
                  <>
                    <ActionButton
                      action={() => onUse(model.path)}
                      className="primary"
                      disabled={inUse || removing[model.id] === true}
                      label={inUse ? "使用中" : "使用"}
                    />
                    <ActionButton
                      action={() => void remove(model)}
                      className="secondary m-0"
                      disabled={removing[model.id] === true}
                      label="删除"
                    />
                  </>
                ) : (
                  <>
                    <ActionButton
                      action={() => void install(model)}
                      className="primary"
                      label={`下载（${formatModelBytes(model.archive_size)}）`}
                    />
                    {client.import && (
                      <ActionButton
                        action={() => void importFiles(model)}
                        className="secondary m-0"
                        label="从文件导入"
                      />
                    )}
                  </>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      {notice && <StatusMessage role="status">{notice}</StatusMessage>}
    </SettingsManagerBlock>
  );
}
