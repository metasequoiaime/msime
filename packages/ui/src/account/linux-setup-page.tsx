import { ControlledCheckbox } from "../core/controlled-checkbox";
import { useEffect, useRef, useState } from "react";
import { linuxSetupFailureMessage } from "./linux-setup-errors";
import { ActionButton } from "../core/action-button";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useInterval } from "../core/use-interval";
import { logo, windowIcons } from "../settings/app-resources";
import * as settings from "../settings/settings-style";
import type { EditionInfo } from "../core/host-capabilities";
import { LinuxInputMethodNotice, type LinuxSetupRegistration } from "./linux-input-method-notice";
import { linuxEditionNames } from "./linux-edition-names";

export type { LinuxSetupRegistration } from "./linux-input-method-notice";

export interface LinuxSetupStatus {
  prepared: boolean;
  stateDirectory: string | null;
  directoryOccupied: boolean;
  setupAvailable: boolean;
  /** 宿主报告过自行下载的词库落后于当前版本（安装包升级只换词库锁、不换下载的词库），页面以「词库需要更新」打开。 */
  dictionaryOutdated?: boolean;
  /** 刚跑完的配置把输入法加入输入法列表的结果。 */
  registration?: LinuxSetupRegistration | null;
  /** 刚跑完的配置要用户留意的提示（脚本的 `msime-setup-note:` 行），配置成功时也显示在结果卡片上。 */
  notes?: string[];
}

export interface LinuxSetupLine {
  text: string;
  error: boolean;
}

/** `msime-linux-setup` 报告的阶段，驱动进度条。`update` 是「立即更新」切换到新词库的那一步。 */
export type LinuxSetupStage =
  | "verify"
  | "download"
  | "prepare"
  | "services"
  | "register"
  | "update";

export interface LinuxSetupChoices {
  download: boolean;
  /** 云候选会把正在输入的拼写发出设备，新装默认关闭；与 Windows 安装器一样在首次配置时询问，勾选才打开。 */
  cloudCandidates: boolean;
}

export interface LinuxDictionaryDownloads {
  directory: string;
  artifacts: {
    name: string;
    url: string | null;
    mirrorUrl: string | null;
    size: number;
    sha256: string;
  }[];
}

export interface LinuxSetupClient {
  /** 从当前安装包读取手动下载清单；只在用户打开手动下载入口时调用。 */
  downloads?: () => Promise<LinuxDictionaryDownloads>;
  /** Runs the packaged setup script; each output line is delivered as it is printed. */
  run: (
    choices: LinuxSetupChoices,
    onLine: (line: LinuxSetupLine) => void,
    onStage?: (stage: LinuxSetupStage) => void,
  ) => Promise<LinuxSetupStatus>;
  /** 「立即更新」：与 `msime-linux-setup --update --download` 相同，取回当前版本的词库、切换代次并回放用户词库。 */
  update?: (
    onLine: (line: LinuxSetupLine) => void,
    onStage?: (stage: LinuxSetupStage) => void,
  ) => Promise<LinuxSetupStatus>;
  /** 标题栏的关闭按钮。窗口没有系统装饰，没有它用户只能靠窗口管理器的快捷键关掉窗口。 */
  close?: () => void | Promise<void>;
  /** 结果卡片上的「打开 Fcitx5 配置」。 */
  openFcitx5Config?: () => Promise<void>;
  /** 结果卡片上的「复制 ibus restart」。 */
  copyText?: (text: string) => Promise<void>;
}

/** 每个阶段开始时进度条至少到哪里、这一阶段最多走到哪里。下载和切换词库没有可报告的进度，进度条在阶段内按剩余距离的一小部分缓慢前进，到下一阶段的起点前停住，不假装已经完成。 */
const stageProgress: Record<LinuxSetupStage | "start", readonly [number, number]> = {
  start: [2, 8],
  verify: [8, 48],
  download: [14, 48],
  prepare: [52, 72],
  services: [74, 86],
  register: [88, 96],
  update: [60, 96],
};

/** 进度条的下一格：先追上这一阶段的起点，再按剩余距离的 3% 向这一阶段的上限挪，每格至少 0.05。 */
export function nextSetupProgress(current: number, stage: LinuxSetupStage | "start"): number {
  const [floor, ceiling] = stageProgress[stage];
  if (current < floor) return floor;
  if (current >= ceiling) return current;
  return Math.min(ceiling, current + Math.max(0.05, (ceiling - current) * 0.03));
}

const stageText: Record<LinuxSetupStage | "start", string> = {
  start: "正在启动配置…",
  verify: "正在校验词库…",
  download: "正在下载词库…",
  prepare: "正在准备输入法配置…",
  services: "正在启用后台服务…",
  register: "正在加入输入法列表…",
  update: "正在切换到新词库，输入会短暂暂停…",
};

type Phase = "ready" | "running" | "done";

const primary =
  "primary m-0 h-11 w-full rounded-[var(--p-r-ctl)] border border-accent bg-accent-strong px-[18px] text-[15px] text-white disabled:opacity-60";

/**
 * Linux 的首次配置窗口，结构与 macOS 的安装窗口（`MacosInstallPage`）一致：居中的小窗口、logo 卡片、标题、选项、主按钮，进行中是带百分比的进度条，结束后是结构化的结果卡片，原始输出收进「显示详细输出」。配色沿用 GNOME 的平台 token。宿主报告词库过期时同一个页面以「词库需要更新」打开，「立即更新」走 `client.update`。
 */
export function LinuxSetupPage({
  status,
  client,
  edition,
  repair = false,
  onComplete,
}: {
  status: LinuxSetupStatus;
  /** 已有配置的资源校验失败时，复用更新事务修复词库，不覆盖用户状态。 */
  repair?: boolean;
  client: LinuxSetupClient;
  /** 本安装包所属的版本（`HostCapabilities.edition`），决定页面上的产品名和终端命令。 */
  edition?: EditionInfo;
  onComplete: () => void;
}) {
  const names = linuxEditionNames(edition);
  const updating = status.prepared && (repair || status.dictionaryOutdated === true);
  const [download, setDownload] = useState(false);
  const [cloudCandidates, setCloudCandidates] = useState(false);
  const [phase, setPhase] = useState<Phase>("ready");
  const [stage, setStage] = useState<LinuxSetupStage | "start">("start");
  const [progress, setProgress] = useState(0);
  const [result, setResult] = useState<LinuxSetupStatus | null>(null);
  const [error, setError] = useState("");
  const [lines, setLines] = useState<LinuxSetupLine[]>([]);
  const [downloads, setDownloads] = useState<LinuxDictionaryDownloads | null>(null);
  const [manualOpen, setManualOpen] = useState(false);
  const [manualError, setManualError] = useState("");
  const [manualLoading, setManualLoading] = useState(false);
  const [copied, setCopied] = useState("");
  const log = useRef<HTMLPreElement>(null);
  const { busy, run } = useAsyncActionRunner(setError, undefined, client);
  const directory = status.stateDirectory ?? "~/.config/msime-client";
  const blocked = updating
    ? client.update
      ? ""
      : linuxSetupFailureMessage({ code: "setup_unavailable" }, names.setupCommand)
    : !status.setupAvailable
      ? linuxSetupFailureMessage({ code: "setup_unavailable" }, names.setupCommand)
      : status.directoryOccupied
        ? `${directory} 已存在但缺少 runtime-options.json。请先移走这个目录，再重新打开设置。`
        : "";

  useEffect(() => {
    log.current?.scrollTo?.({ top: log.current.scrollHeight });
  }, [lines]);

  useInterval(
    () => setProgress((current) => nextSetupProgress(current, stage)),
    phase === "running" ? 120 : null,
  );

  const start = async () => {
    if (busy) return;
    setError("");
    setLines([]);
    setStage("start");
    setProgress(0);
    setPhase("running");
    await run(
      async (isCurrent) => {
        const onLine = (line: LinuxSetupLine) => {
          if (isCurrent()) setLines((current) => [...current, line]);
        };
        const onStage = (next: LinuxSetupStage) => {
          if (isCurrent()) setStage(next);
        };
        const outcome =
          updating && client.update
            ? await client.update(onLine, onStage)
            : await client.run({ download, cloudCandidates }, onLine, onStage);
        if (!isCurrent()) return;
        if (outcome.prepared) {
          setProgress(100);
          setResult(outcome);
          setPhase("done");
        } else {
          setPhase("ready");
          setError(linuxSetupFailureMessage(null, names.setupCommand));
        }
      },
      {
        formatError: (error) => linuxSetupFailureMessage(error, names.setupCommand),
        onError: () => setPhase("ready"),
      },
    );
  };

  const showDownloads = async () => {
    setManualOpen(true);
    if (downloads || manualLoading || !client.downloads) return;
    setManualError("");
    setManualLoading(true);
    try {
      setDownloads(await client.downloads());
    } catch {
      setManualError(
        `未能读取下载清单。可在终端运行 ${names.setupCommand} --list-downloads 查看；自动配置仍可使用。`,
      );
    } finally {
      setManualLoading(false);
    }
  };
  const copyDownloadText = async (text: string, label: string) => {
    if (!client.copyText) return;
    setCopied("");
    try {
      await client.copyText(text);
      setCopied(`已复制${label}`);
    } catch {
      setCopied("复制失败，请选中文字手动复制。");
    }
  };

  const percent = Math.floor(progress);
  const action = updating ? "更新" : "配置";
  const failed = Boolean(error) && phase === "ready";
  return (
    <main
      className="relative flex h-full min-h-full w-full flex-col bg-[var(--p-bg)] text-[var(--p-text)] select-none [font-family:var(--p-font)]"
      aria-label={updating ? "更新词库" : "首次配置"}
      data-onboarding-shell=""
      data-platform="linux"
    >
      {/* 窗口没有系统装饰：这条 GNOME 式的标题栏用来拖动窗口，右侧是关闭按钮。 */}
      <header
        className="relative flex h-[46px] shrink-0 items-center justify-center border-b border-[var(--p-hair)] bg-[var(--p-chrome)]"
        data-tauri-drag-region=""
      >
        <span className="pointer-events-none text-[15px] font-bold">{names.name}</span>
        {client.close && (
          <span className={`${settings.windowControls} absolute right-0`}>
            <ActionButton
              action={() => void client.close?.()}
              className={settings.windowClose}
              ariaLabel="关闭"
              label={
                <img
                  className={settings.windowIcon}
                  src={windowIcons.close}
                  alt=""
                  draggable={false}
                />
              }
            />
          </span>
        )}
      </header>
      <div className="flex min-h-0 flex-1 flex-col items-center overflow-y-auto px-8 pt-7 pb-6">
        <img src={logo} alt="" className="size-[88px] shrink-0" />
        <h1 className="mt-5 mb-0 text-[24px] font-[650] tracking-[0.02em]">{names.name}</h1>
        <h2
          className={`mt-3 mb-0 text-[15px] font-semibold ${failed ? "text-danger" : ""}`}
          aria-live="polite"
        >
          {phase === "done"
            ? updating
              ? "词库已更新"
              : "配置完成"
            : failed
              ? `${action}没有完成`
              : updating
                ? repair
                  ? "运行资源需要修复"
                  : "词库需要更新"
                : "准备词库和配置"}
        </h2>
        <p className="mt-2 mb-0 max-w-[400px] text-center text-[13px] leading-relaxed text-[var(--p-sub)]">
          {phase === "done"
            ? updating
              ? "已切换到当前版本的词库，用户词库已一并迁移，新的输入即使用新词库。"
              : "输入法配置已就绪。"
            : updating
              ? repair
                ? "将重新校验并取回当前版本的词库，保留已有偏好和用户词库。"
                : "已安装的词库早于当前版本，输入法暂时继续使用旧词库。立即更新会取回新词库并切换，用户词库会一并迁移。"
              : `首次使用需要校验词库，并在 ${directory} 创建输入法配置。拼音切分、候选排序和词频学习都在本机完成。`}
        </p>
        {phase === "ready" && !updating && !blocked && (
          <div className="mt-5 flex w-full max-w-[400px] flex-col gap-2.5 text-left text-xs leading-[1.7] text-[var(--p-sub)] select-text">
            <label className="flex items-start gap-2">
              <ControlledCheckbox checked={download} disabled={busy} onChange={setDownload} />
              <span>词库不完整时从固定地址下载（首次约 270 MB）</span>
            </label>
            <label className="flex items-start gap-2">
              <ControlledCheckbox
                checked={cloudCandidates}
                disabled={busy}
                onChange={setCloudCandidates}
              />
              <span>
                启用云候选：输入过程中把正在输入的拼写通过 HTTPS 发送给 Google 的 input-tools
                服务（inputtools.google.com），换回一条额外候选。已上屏的文本、词库内容和学习到的词频都不会发送。默认不启用，之后可在设置里更改。
              </span>
            </label>
          </div>
        )}
        {phase === "ready" && !updating && client.downloads && (
          <div className="mt-4 w-full max-w-[400px] text-left text-xs leading-relaxed select-text">
            <button
              type="button"
              className={settings.backLink}
              onClick={() => (manualOpen ? setManualOpen(false) : void showDownloads())}
              aria-expanded={manualOpen}
            >
              手动下载词库
            </button>
            {manualOpen && (
              <div className="mt-2 space-y-3 text-[var(--p-sub)]">
                <p>
                  可复制链接交给浏览器或其他下载工具。将全部文件按原名放入以下目录，取消自动下载后点击「开始配置」，安装器会核对文件大小和
                  SHA-256。
                </p>
                {manualLoading && <p role="status">正在读取下载清单…</p>}
                {manualError && (
                  <p role="alert">
                    {manualError}{" "}
                    <button
                      type="button"
                      className={settings.backLink}
                      onClick={() => void showDownloads()}
                    >
                      重试读取
                    </button>
                  </p>
                )}
                {downloads && (
                  <>
                    <p className="break-all">
                      保存目录：<code>{downloads.directory}</code>
                    </p>
                    {client.copyText && (
                      <div className="flex flex-wrap gap-3">
                        <button
                          type="button"
                          className={settings.backLink}
                          onClick={() => void copyDownloadText(downloads.directory, "保存目录")}
                        >
                          复制保存目录
                        </button>
                        <button
                          type="button"
                          className={settings.backLink}
                          disabled={!downloads.artifacts.some((file) => file.url)}
                          onClick={() =>
                            void copyDownloadText(
                              downloads.artifacts
                                .flatMap((file) => (file.url ? [file.url] : []))
                                .join("\n"),
                              "原地址",
                            )
                          }
                        >
                          复制全部原地址
                        </button>
                      </div>
                    )}
                    <ul className="m-0 list-none space-y-3 p-0">
                      {downloads.artifacts.map((file) => (
                        <li key={file.name} className="space-y-1 break-all">
                          <strong className="text-[var(--p-text)]">{file.name}</strong>（
                          {file.size.toLocaleString()} 字节）
                          <div>
                            SHA-256：<code>{file.sha256}</code>
                          </div>
                          <div>
                            原地址：<span>{file.url ?? "本安装包未提供下载地址"}</span>
                          </div>
                          {file.mirrorUrl && (
                            <div>
                              国内镜像：<span>{file.mirrorUrl}</span>
                            </div>
                          )}
                          {client.copyText && file.url && (
                            <div className="flex gap-3">
                              <button
                                type="button"
                                className={settings.backLink}
                                onClick={() =>
                                  void copyDownloadText(file.url!, file.name + "原地址")
                                }
                              >
                                复制原地址
                              </button>
                              {file.mirrorUrl && (
                                <button
                                  type="button"
                                  className={settings.backLink}
                                  onClick={() =>
                                    void copyDownloadText(file.mirrorUrl!, file.name + "镜像地址")
                                  }
                                >
                                  复制国内镜像
                                </button>
                              )}
                            </div>
                          )}
                        </li>
                      ))}
                    </ul>
                  </>
                )}
                {copied && <p role="status">{copied}</p>}
              </div>
            )}
          </div>
        )}
        {phase === "done" && !updating && (
          <div className="mt-5 flex w-full max-w-[400px]">
            <LinuxInputMethodNotice
              registration={result?.registration}
              names={names}
              notes={result?.notes}
              openFcitx5Config={client.openFcitx5Config}
              copyText={client.copyText}
            />
          </div>
        )}
        <div className="mt-7 flex w-[260px] shrink-0 flex-col items-stretch">
          {phase === "running" ? (
            <>
              <div
                role="progressbar"
                aria-label={`正在${action}`}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={percent}
                className="relative h-11 overflow-hidden rounded-[var(--p-r-ctl)] bg-accent-soft text-[15px]"
              >
                <div
                  className="absolute inset-y-0 left-0 bg-accent-strong transition-[width] duration-100 ease-linear"
                  style={{ width: `${progress}%` }}
                />
                {/* 两份文字：空的部分用强调色，填充上面用白色，任何百分比下都看得清。 */}
                <span className="absolute inset-0 grid place-items-center text-accent tabular-nums">
                  正在{action} {percent}%
                </span>
                <span
                  aria-hidden="true"
                  className="absolute inset-0 grid place-items-center text-white tabular-nums"
                  style={{ clipPath: `inset(0 ${100 - progress}% 0 0)` }}
                >
                  正在{action} {percent}%
                </span>
              </div>
              <p className="mt-2 mb-0 text-center text-xs text-[var(--p-sub)]" aria-live="polite">
                {stageText[stage]}
              </p>
            </>
          ) : phase === "done" ? (
            <ActionButton action={onComplete} className={primary} label="进入设置" />
          ) : (
            <>
              <ActionButton
                action={() => void start()}
                className={primary}
                disabled={busy || Boolean(blocked)}
                label={
                  updating
                    ? error
                      ? "重试"
                      : repair
                        ? "立即修复"
                        : "立即更新"
                    : error
                      ? "重新配置"
                      : "开始配置"
                }
              />
              {updating && (
                <ActionButton
                  action={onComplete}
                  className="mt-2 h-9 rounded-[var(--p-r-ctl)] border-0 bg-transparent text-[13px] text-[var(--p-sub)] hover:bg-[var(--p-hover)]"
                  label="稍后"
                />
              )}
            </>
          )}
        </div>
        {(error || blocked) && (
          <p
            className="mt-4 mb-0 max-w-[400px] text-center text-[13px] leading-relaxed text-danger"
            role="alert"
          >
            {error || blocked}
          </p>
        )}
        {lines.length > 0 && (
          <details className="mt-5 w-full max-w-[400px] text-xs" open={failed}>
            <summary className="cursor-pointer text-[var(--p-sub)]">显示详细输出</summary>
            <pre
              ref={log}
              role="log"
              aria-label="配置输出"
              className="mt-2 mb-0 max-h-[200px] overflow-auto rounded-[var(--p-r-ctl)] border border-[var(--p-hair)] bg-[var(--p-group-bg)] p-3 leading-relaxed whitespace-pre-wrap select-text"
            >
              {lines.map((line, index) => (
                <span key={index} className={line.error ? "text-danger" : undefined}>
                  {line.text}
                  {"\n"}
                </span>
              ))}
            </pre>
          </details>
        )}
        <p className="mt-auto mb-0 max-w-[400px] pt-5 text-center text-xs leading-[1.7] text-[var(--p-sub)]">
          {updating
            ? `也可以在终端运行 ${names.setupCommand} --update --download 完成同样的更新。`
            : `也可以在终端运行 ${names.setupCommand} 完成同样的配置；配置文件、凭据和学习数据只保存在本机。`}
        </p>
      </div>
    </main>
  );
}
