import { useEffect, useRef, useState } from "react";
import * as onboarding from "./onboarding-style";
import { linuxSetupFailureMessage } from "./linux-setup-errors";
import { ActionButton } from "../core/action-button";

export interface LinuxSetupStatus {
  prepared: boolean;
  stateDirectory: string | null;
  directoryOccupied: boolean;
  setupAvailable: boolean;
}

export interface LinuxSetupLine {
  text: string;
  error: boolean;
}

export interface LinuxSetupChoices {
  download: boolean;
  /** Cloud candidates are the one network feature active without any token, so the first setup asks, as the Windows installer does. */
  cloudCandidates: boolean;
}

export interface LinuxSetupClient {
  /** Runs the packaged setup script; each output line is delivered as it is printed. */
  run: (
    choices: LinuxSetupChoices,
    onLine: (line: LinuxSetupLine) => void,
  ) => Promise<LinuxSetupStatus>;
}

export function LinuxSetupPage({
  status,
  client,
  onComplete,
}: {
  status: LinuxSetupStatus;
  client: LinuxSetupClient;
  onComplete: () => void;
}) {
  const [download, setDownload] = useState(false);
  const [cloudCandidates, setCloudCandidates] = useState(true);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const [error, setError] = useState("");
  const [lines, setLines] = useState<LinuxSetupLine[]>([]);
  const log = useRef<HTMLPreElement>(null);
  const mounted = useRef(true);
  const actionRunning = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const directory = status.stateDirectory ?? "~/.config/msime-client";
  const blocked = !status.setupAvailable
    ? linuxSetupFailureMessage({ code: "setup_unavailable" })
    : status.directoryOccupied
      ? `${directory} 已存在但缺少 runtime-options.json。请先移走这个目录，再重新打开设置。`
      : "";

  useEffect(() => {
    log.current?.scrollTo?.({ top: log.current.scrollHeight });
  }, [lines]);

  const start = async () => {
    if (busy || actionRunning.current || !mounted.current) return;
    actionRunning.current = true;
    setBusy(true);
    setError("");
    setLines([]);
    try {
      const result = await client.run({ download, cloudCandidates }, (line) => {
        if (mounted.current) setLines((current) => [...current, line]);
      });
      if (!mounted.current) return;
      if (result.prepared) setDone(true);
      else setError(linuxSetupFailureMessage(null));
    } catch (failure) {
      if (mounted.current) setError(linuxSetupFailureMessage(failure));
    } finally {
      actionRunning.current = false;
      if (mounted.current) setBusy(false);
    }
  };

  return (
    <main
      className={`${onboarding.page} ${onboarding.buttons}`}
      aria-label="首次配置"
      data-onboarding-shell=""
      data-platform="linux"
    >
      <header className={onboarding.header}>
        <img src={new URL("./assets/msime.svg", import.meta.url).href} alt="" />
        <h1>水杉输入法</h1>
      </header>
      <div className={onboarding.body}>
        <section className={onboarding.section}>
          <h2 className={onboarding.sectionTitle}>{done ? "配置完成" : "准备词库和配置"}</h2>
          <p className={onboarding.lead}>
            {done
              ? "上面输出的最后几行说明了输入法列表的情况：已自动加入 Fcitx5 或 IBus 的输入法列表时，用输入法切换快捷键切换过去即可开始输入；未能自动加入时，按其中的下一步手动添加。"
              : `首次使用需要校验词库，并在 ${directory} 创建输入法配置。拼音切分、候选排序和词频学习都在本机完成。`}
          </p>
          {!done && !blocked && (
            <label className={onboarding.note}>
              <input
                type="checkbox"
                checked={download}
                disabled={busy}
                onChange={(event) => setDownload(event.target.checked)}
              />{" "}
              词库不完整时从固定地址下载（首次约 170 MB）
            </label>
          )}
          {!done && !blocked && (
            <label className={onboarding.note}>
              <input
                type="checkbox"
                checked={cloudCandidates}
                disabled={busy}
                onChange={(event) => setCloudCandidates(event.target.checked)}
              />{" "}
              启用云候选：输入过程中把正在输入的拼写通过 HTTPS 发送给 Google 的 input-tools
              服务（inputtools.google.com），换回一条额外候选。已上屏的文本、词库内容和学习到的词频都不会发送。这是唯一一项配置完就会联网的功能，之后可在设置里更改。
            </label>
          )}
          {lines.length > 0 && (
            <pre
              ref={log}
              role="log"
              aria-label="配置输出"
              className="m-0 max-h-[260px] overflow-auto rounded-xl border border-edge bg-card p-3 text-xs leading-relaxed whitespace-pre-wrap"
            >
              {lines.map((line, index) => (
                <span key={index} className={line.error ? "text-danger" : undefined}>
                  {line.text}
                  {"\n"}
                </span>
              ))}
            </pre>
          )}
          <p className={onboarding.note}>
            也可以在终端运行 msime-linux-setup
            完成同样的配置；配置文件、凭据和学习数据只保存在本机。
          </p>
        </section>
      </div>
      {(error || blocked) && (
        <p className={onboarding.error} role="alert">
          {error || blocked}
        </p>
      )}
      <footer className={onboarding.footer}>
        {done ? (
          <ActionButton
            action={onComplete}
            className={`primary ${onboarding.next}`}
            label="进入设置"
          />
        ) : (
          <ActionButton
            action={() => void start()}
            className={`primary ${onboarding.next}`}
            disabled={busy || Boolean(blocked)}
            label={busy ? "正在配置…" : error ? "重新配置" : "开始配置"}
          />
        )}
      </footer>
    </main>
  );
}
