import { useCallback, useEffect, useRef, useState } from "react";
import type { InputSourceStartupStatus } from "../settings/input-source-startup-notice";
import { ActionButton } from "../core/action-button";

export interface MacosInstallClient {
  /** Copies the input method into `~/Library/Input Methods` and registers it; resolves with the result once both have finished. */
  install: () => Promise<InputSourceStartupStatus>;
}

/**
 * How the bar moves while the install runs. Copying and registering report no progress of their own and take a few seconds, so the bar is paced rather than measured: it closes a share of the remaining gap to `HOLD` on each tick, with uneven steps and the occasional pause, and only goes past `HOLD` once the install has actually finished. `MINIMUM_MS` keeps an install that finishes at once from flashing the bar.
 */
const TICK_MS = 90;
const HOLD = 93;
const MINIMUM_MS = 2200;
const FINISH_STEP = 7;

export function nextSimulatedProgress(current: number, finished: boolean, random: number): number {
  if (finished) return Math.min(100, current + FINISH_STEP);
  // About one tick in six stands still, as a real copy does on a large file.
  if (random < 0.16) return current;
  const share = 0.035 + random * 0.065;
  return Math.min(HOLD, current + Math.max(0.25, (HOLD - current) * share));
}

type Phase = "ready" | "installing" | "done";

function resultTitle(status: InputSourceStartupStatus): string {
  return status.action === "failed" ? "安装没有完成" : "安装完成";
}

/** Adding the input source to the System Settings list is left to the settings page's status notice, which also covers a login that has to come first; this only points there. */
function resultLead(status: InputSourceStartupStatus): string {
  if (status.action === "failed") return "没能把输入法安装到本机，可以再试一次。";
  if (status.action === "login_required")
    return "注销并重新登录后才能在系统设置里添加它，设置页里有具体步骤。";
  if (status.enabled === true)
    return "按 Control+空格 或在菜单栏的输入法菜单中切换到水杉输入法即可开始输入。";
  return "进入设置后，按页面上的提示把它添加到系统的输入法列表。";
}

/** The macOS first-install window: the settings app opens as this on a machine that has never had the input method, and makes way for the settings page once the user leaves it. */
export function MacosInstallPage({
  client,
  onComplete,
}: {
  client: MacosInstallClient;
  onComplete: () => void;
}) {
  const [phase, setPhase] = useState<Phase>("ready");
  const [progress, setProgress] = useState(0);
  const [result, setResult] = useState<InputSourceStartupStatus | null>(null);
  const outcome = useRef<InputSourceStartupStatus | null>(null);
  const startedAt = useRef(0);

  useEffect(() => {
    if (phase !== "installing") return;
    const timer = window.setInterval(() => {
      const finished = outcome.current !== null && Date.now() - startedAt.current >= MINIMUM_MS;
      setProgress((current) => nextSimulatedProgress(current, finished, Math.random()));
    }, TICK_MS);
    return () => window.clearInterval(timer);
  }, [phase]);

  // The bar only reaches 100 once the install has finished; let the full bar show for a moment before it turns into the result.
  useEffect(() => {
    const status = outcome.current;
    if (phase !== "installing" || progress < 100 || !status) return;
    const timer = window.setTimeout(() => {
      setResult(status);
      setPhase("done");
    }, 260);
    return () => window.clearTimeout(timer);
  }, [phase, progress]);

  const install = useCallback(() => {
    if (phase === "installing") return;
    outcome.current = null;
    startedAt.current = Date.now();
    setResult(null);
    setProgress(0);
    setPhase("installing");
    void client
      .install()
      .catch((): InputSourceStartupStatus => ({
        action: "failed",
        enabled: null,
        bundled_version: null,
        installed_version: null,
      }))
      .then((status) => {
        outcome.current = status;
      });
  }, [client, phase]);

  const percent = Math.floor(progress);
  return (
    <main
      className="relative flex h-full min-h-full w-full flex-col items-center justify-center overflow-y-auto bg-chrome px-8 pt-12 pb-10 text-body select-none"
      aria-label="安装水杉输入法"
      data-platform="macos"
    >
      {/* The window has no title bar of its own; this strip under the traffic lights moves it. */}
      <div className="absolute inset-x-0 top-0 h-10" data-tauri-drag-region="" />
      <div className="grid size-[88px] place-items-center rounded-[22px] bg-raised shadow-card">
        <img
          src={new URL("../assets/msime.svg", import.meta.url).href}
          alt=""
          className="size-[60px]"
        />
      </div>
      <h1 className="mt-5 mb-0 text-[24px] font-[650] tracking-[0.02em]">水杉输入法</h1>
      {phase === "done" && result ? (
        <>
          <h2
            className={`mt-3 mb-0 text-[15px] font-semibold ${result.action === "failed" ? "text-danger" : ""}`}
            role={result.action === "failed" ? "alert" : "status"}
          >
            {resultTitle(result)}
          </h2>
          <p className="mt-2 mb-0 max-w-[340px] text-center text-[13px] leading-relaxed text-secondary">
            {resultLead(result)}
          </p>
        </>
      ) : (
        <p className="mt-2 mb-0 text-[13px] text-secondary">适用于 macOS 13 及以上版本</p>
      )}
      <div className="mt-11 flex w-[260px] flex-col items-stretch">
        {phase === "installing" ? (
          <div
            role="progressbar"
            aria-label="正在安装"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
            className="relative h-11 overflow-hidden rounded-[10px] bg-accent-soft text-[15px]"
          >
            <div
              className="absolute inset-y-0 left-0 bg-accent-strong transition-[width] duration-100 ease-linear"
              style={{ width: `${progress}%` }}
            />
            {/* Two copies of the label: accent on the empty part, white over the fill, so it reads at any percentage. */}
            <span className="absolute inset-0 grid place-items-center text-accent tabular-nums">
              正在安装 {percent}%
            </span>
            <span
              aria-hidden="true"
              className="absolute inset-0 grid place-items-center text-white tabular-nums"
              style={{ clipPath: `inset(0 ${100 - progress}% 0 0)` }}
            >
              正在安装 {percent}%
            </span>
          </div>
        ) : phase === "ready" ? (
          <ActionButton action={install} className={primary} label="立即安装" />
        ) : result?.action === "failed" ? (
          <ActionButton action={install} className={primary} label="重试" />
        ) : (
          <ActionButton action={onComplete} className={primary} label="进入设置" />
        )}
      </div>
    </main>
  );
}

const primary =
  "primary m-0 h-11 rounded-[10px] border border-accent bg-accent-strong px-[18px] text-[15px] text-white";
