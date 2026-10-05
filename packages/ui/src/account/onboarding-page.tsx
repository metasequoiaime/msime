import { useCallback, useEffect, useRef, useState } from "react";
import { Switch } from "../core/platform-controls";
import { ActionButton } from "../core/action-button";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useSettingsPlatform } from "../theme/settings-platform";
import type { EditionInfo } from "../index";
import * as onboarding from "./onboarding-style";

/** 首次设置选用的触屏键盘。全拼 26 键和 9 键由用户在「选择输入方式」一步里选；不提供全拼的版本跳过这一步，直接用版本默认方案的键盘（五笔版是五笔）。 */
export type OnboardingInputScheme = "quanpin" | "nine_key" | "wubi";

/** 不提供全拼的版本直接使用的键盘；提供全拼（包括 full 和没有版本信息时）返回 undefined，由用户在「选择输入方式」一步里选。 */
export function onboardingEditionScheme(edition?: EditionInfo): OnboardingInputScheme | undefined {
  if (!edition || edition.input_schemes.includes("quanpin")) return undefined;
  return edition.default_scheme === "wubi" ? "wubi" : undefined;
}

/** What the flow asks of the host once the user finishes it, beyond the scheme. */
export interface OnboardingChoices {
  /** The 候选下方显示译文 switch, or undefined when the user never touched it, so a replayed flow does not turn off a gloss someone had switched on in settings. */
  candidateEnglishGloss?: boolean;
  /** True when the user chose to sign in: the host opens 我的 instead of the default page. */
  openAccount: boolean;
}

export interface OnboardingActions {
  platform?: "android" | "harmony" | "ios";
  /** The host's `mobile_settings` capability. A HarmonyOS 2-in-1 reports false: it is a desktop, so the flow takes the desktop look (a centred sheet with a button bar) and never opens on the splash. */
  mobileSettings?: boolean;
  prepareResources: () => Promise<void>;
  openSystemKeyboardSettings: () => Promise<void>;
  showInputMethodPicker: () => Promise<void>;
}

/** How long the first-launch splash stays up before the flow opens on its own. */
export const SPLASH_MS = 2800;
/** The panel the mark sits in (the frame of msime_frame.svg without its brush-scatter texture, which is a 200KB raster-like path set). */
const framePath = "M5.84314 5.8335H109.843V125.833H5.84314V5.8335Z";
/** The MSIME mark as a single stroke, so the splash can draw it. */
const logoPath =
  "M80.394 18.8335L34.3451 36.489L80.394 49.7306L34.3451 71.7999C77.8789 79.1564 118.8 85.1887 31.8431 113.088";

const stepTitles = [
  "把水杉加进键盘",
  "选择输入方式",
  "候选下方显示译文",
  "登录后多端同步",
] as const;

export function SetupStep({
  number,
  title,
  children,
  last,
}: {
  number: number;
  title: string;
  children: string;
  last?: boolean;
}) {
  return (
    <div className={onboarding.setupStep}>
      <span className={onboarding.setupNumber} aria-hidden="true">
        {number}
      </span>
      <span>
        <strong>{title}</strong>
        <small>{children}</small>
      </span>
      {!last && <i aria-hidden="true" />}
    </div>
  );
}

/** The splash: the mark draws itself, then the flow opens. A tap anywhere skips it. */
function Splash({ onDone }: { onDone: () => void }) {
  useEffect(() => {
    const timer = window.setTimeout(onDone, SPLASH_MS);
    return () => window.clearTimeout(timer);
  }, [onDone]);
  return (
    <button type="button" className={onboarding.splash} aria-label="跳过开屏" onClick={onDone}>
      <span className={onboarding.splashGlow} aria-hidden="true" />
      <svg className={onboarding.splashLogo} viewBox="0 0 116 132" aria-hidden="true">
        <path className={onboarding.splashFrame} d={framePath} />
        <path className={onboarding.splashStroke} d={logoPath} pathLength={1} />
      </svg>
      <span className={onboarding.splashName} aria-hidden="true">
        水杉输入法
      </span>
      <span className={onboarding.splashTagline} aria-hidden="true">
        METASEQUOIA IME
      </span>
      <span className={onboarding.splashHint} aria-hidden="true">
        轻点跳过
      </span>
    </button>
  );
}

/** What signing in brings, in the design's order; each has a backend behind it. */
const perks = [
  ["▤", "词库"],
  ["◈", "皮肤与主题"],
  ["⇅", "云剪贴板"],
] as const;

const glossSamples = [
  ["你好", "hello"],
  ["您好", "hello"],
  ["你们好", "hello, all"],
] as const;

export function WelcomeFlowPage({
  actions,
  onComplete,
  onSkip,
  splash = false,
  edition,
}: {
  actions: OnboardingActions;
  onComplete: (scheme: OnboardingInputScheme, choices: OnboardingChoices) => Promise<void>;
  onSkip?: () => Promise<void>;
  /** Opens on the splash. Only a first launch passes it; a replay from settings goes straight to the steps. */
  splash?: boolean;
  /** 运行中的版本（`HostCapabilities.edition`），不是 full 时才有。不提供全拼的版本没有「选择输入方式」这一步。 */
  edition?: EditionInfo;
}) {
  const editionScheme = onboardingEditionScheme(edition);
  // 实际经过的步骤：`page` 仍是步骤的编号（对应 `stepTitles`），进度按经过的步骤计。
  const steps: readonly number[] = editionScheme ? [0, 2, 3] : [0, 1, 2, 3];
  const [requestedPage, setPage] = useState(0);
  // 第一次启动时版本要等第一步准备好资源（写下 HostOptions）之后才知道，`edition` 可能在流程中途才到：落在本版本没有的步骤上时显示它之后的那一步。
  const page = steps.includes(requestedPage)
    ? requestedPage
    : (steps.find((candidate) => candidate > requestedPage) ?? 3);
  const step = steps.indexOf(page);
  const [scheme, setScheme] = useState<OnboardingInputScheme>(editionScheme ?? "quanpin");
  const [gloss, setGloss] = useState<boolean>();
  const [error, setError] = useState("");
  const prepared = useRef(false);
  const { busy, run: runAsyncAction } = useAsyncActionRunner(setError, undefined);
  const platform = useSettingsPlatform({
    platform: actions.platform ?? "android",
    mobile_settings: actions.mobileSettings,
  });
  const ios = actions.platform === "ios";
  const harmony = actions.platform === "harmony";
  const android = platform === "android";
  const ipad = platform === "ipad";
  // A HarmonyOS 2-in-1 is a desktop: the design gives it no splash and draws the flow as a modal sheet with a desktop button bar.
  const desktop = platform === "hm2";
  const [splashing, setSplashing] = useState(splash && !desktop);
  const endSplash = useCallback(() => setSplashing(false), []);

  const run = async (operation: () => Promise<void>, next?: number) => {
    if (busy) return;
    await runAsyncAction(
      async (isCurrent) => {
        await operation();
        if (isCurrent() && next !== undefined) setPage(next);
      },
      { formatError: () => "操作失败，请稍后重试。" },
    );
  };

  // The built-in dictionaries have to be in place before the keyboard is enabled, so they are prepared once, ahead of whichever comes first: a system-settings action or leaving the step.
  const ensureResources = async () => {
    if (prepared.current) return;
    await actions.prepareResources();
    prepared.current = true;
  };
  const withResources = (action: () => Promise<void>) => async () => {
    await ensureResources();
    await action();
  };
  const finish = (openAccount: boolean) =>
    void run(() =>
      onComplete(editionScheme ?? scheme, { candidateEnglishGloss: gloss, openAccount }),
    );
  // Skipping leaves the walkthrough, not the preparation: the keyboard still needs its dictionaries, and on Android the flow keeps coming back until they are in place.
  const skipFlow = onSkip
    ? () =>
        void run(async () => {
          await ensureResources();
          await onSkip();
        })
    : undefined;

  const advance = () => {
    // 准备之后才可能知道版本，下一步按准备之后的步骤定（见 `page`）。
    if (page === 0) void run(ensureResources, 1);
    else if (page === 3) finish(true);
    else setPage(steps[step + 1] ?? 3);
  };
  // The last step's action is signing in (dc.html `STEPS[3].cta`); leaving without it is the 稍后再说 next to it.
  const nextLabel = busy ? "正在准备…" : page === 3 ? "登录" : "下一步";

  const shell = {
    "aria-label": "首次设置",
    "data-onboarding-shell": "",
    "data-mobile": desktop ? undefined : "",
    "data-platform": platform,
  };

  if (splashing)
    return (
      // Marked the same way as the flow, so the splash takes the brand palette rather than the desktop one.
      <main className={onboarding.splashShell} {...shell}>
        <Splash onDone={endSplash} />
      </main>
    );

  const skip = skipFlow && (
    <ActionButton action={skipFlow} className={onboarding.skip} disabled={busy} label="跳过" />
  );
  const later = (
    <ActionButton
      action={() => finish(false)}
      className={android ? onboarding.textButton : desktop ? onboarding.deskLink : onboarding.later}
      disabled={busy}
      label="稍后再说"
    />
  );
  const back = (
    <ActionButton
      action={() => setPage(steps[step - 1] ?? 0)}
      className={android ? onboarding.textButton : desktop ? "secondary" : onboarding.back}
      disabled={busy}
      label="上一步"
    />
  );
  const next = (
    <ActionButton
      action={advance}
      className={android || desktop ? "primary" : `primary ${onboarding.next}`}
      disabled={busy}
      label={nextLabel}
    />
  );

  const footer = android ? (
    // Material's pair: the text button on the leading edge is 跳过 on the first step and 上一步 after it (dc.html `ob.andLeft`).
    <footer className={onboarding.pairFooter}>
      {page > 0 ? (
        back
      ) : skipFlow ? (
        <ActionButton
          action={skipFlow}
          className={onboarding.textButton}
          disabled={busy}
          label="跳过"
        />
      ) : (
        <span />
      )}
      <span className={onboarding.pairEnd}>
        {page === 3 && later}
        {next}
      </span>
    </footer>
  ) : desktop ? (
    // The desktop bar (dc.html `onbDeskBar`): leave on the left, then 上一步 and the primary action on the right.
    <footer className={onboarding.deskFooter}>
      {page === 3
        ? later
        : skipFlow && (
            <ActionButton
              action={skipFlow}
              className={onboarding.deskLink}
              disabled={busy}
              label="跳过"
            />
          )}
      <span className={onboarding.deskSpacer} />
      {page > 0 && back}
      {next}
    </footer>
  ) : (
    <footer className={onboarding.footer}>
      {/* A plain div's aria-label is not announced; as an image the dots read as the count they draw. Not a progressbar: that role belongs to the linear bar Android draws in their place. */}
      <div
        className={onboarding.dots}
        role="img"
        aria-label={`第 ${step + 1} 步，共 ${steps.length} 步`}
      >
        {steps.map((index) => (
          <i key={index} className={onboarding.dot(index === page)} />
        ))}
      </div>
      {next}
      {page === 3 && later}
      {page > 0 && back}
    </footer>
  );

  const content = (
    <>
      {android && (
        <div
          className={onboarding.progressTrack}
          role="progressbar"
          aria-label="设置进度"
          aria-valuemin={1}
          aria-valuemax={steps.length}
          aria-valuenow={step + 1}
        >
          <i
            className={onboarding.progressFill}
            style={{ width: `${((step + 1) / steps.length) * 100}%` }}
          />
        </div>
      )}
      <header className={onboarding.header}>
        <img src={new URL("./assets/msime.svg", import.meta.url).href} alt="" />
        <div>
          {/* Android shows its progress as the bar above; the desktop spells the count out (dc.html `ob.countTxt`). */}
          {!android && (
            <p className={onboarding.progress}>
              {desktop
                ? `第 ${step + 1} 步，共 ${steps.length} 步`
                : `${step + 1} / ${steps.length}`}
            </p>
          )}
          <h1>{stepTitles[page]}</h1>
        </div>
        {/* The phones and the iPad keep 跳过 at the top (dc.html `ob.topSkip`); Android and the desktop have it in the button bar. */}
        {!android && !desktop && skip}
      </header>
      <div className={onboarding.body}>
        {page === 0 && (
          <section className={onboarding.section}>
            <h2 className={onboarding.sectionTitle}>{ios ? "添加水杉键盘" : "添加水杉输入法"}</h2>
            <p className={onboarding.lead}>
              {ios
                ? "在系统键盘列表中启用水杉，再回到任意输入框开始使用。"
                : harmony
                  ? "准备好内置词库后，按下面步骤在 HarmonyOS 中启用并选择水杉输入法。"
                  : "准备好内置词库后，按下面步骤启用系统键盘。"}
            </p>
            <div className={onboarding.setupCard}>
              <SetupStep number={1} title="打开键盘设置">
                {ios
                  ? "前往系统设置中的“通用 → 键盘 → 键盘”。"
                  : harmony
                    ? "前往 HarmonyOS 的系统输入法设置。"
                    : "前往系统设置中的“语言和输入法”或“屏幕键盘”。"}
              </SetupStep>
              <SetupStep number={2} title={ios ? "添加水杉键盘" : "启用水杉输入法"}>
                {ios
                  ? "在第三方键盘列表中添加水杉键盘。"
                  : "在可用输入法列表中打开 MSIME Preview。"}
              </SetupStep>
              <SetupStep number={3} title="切换并开始输入" last>
                {ios
                  ? "在输入框中切换到水杉键盘即可开始使用。"
                  : "在输入框中选择水杉输入法即可开始使用。"}
              </SetupStep>
            </div>
            <div className={onboarding.systemActions}>
              <ActionButton
                action={() => void run(withResources(actions.openSystemKeyboardSettings))}
                className="primary"
                disabled={busy}
                label="打开系统设置"
              />
              {!ios && (
                <ActionButton
                  action={() => void run(withResources(actions.showInputMethodPicker))}
                  className="secondary"
                  disabled={busy}
                  label="选择输入法"
                />
              )}
            </div>
            <p className={onboarding.note}>
              {ios
                ? "系统设置页面由 iOS 管理，水杉不会自动启用或切换键盘。"
                : harmony
                  ? "系统设置页面由 HarmonyOS 管理，水杉不会自动启用或切换输入法。"
                  : "系统设置页面由 Android 管理，水杉不会自动启用或切换输入法。"}
            </p>
          </section>
        )}
        {page === 1 && (
          <section className={onboarding.section}>
            <h2 className={onboarding.sectionTitle}>从你熟悉的键盘开始</h2>
            <p className={onboarding.lead}>先选一种，稍后可以在输入设置中调整全部方案。</p>
            <div className={onboarding.schemeList} role="radiogroup" aria-label="首次输入方案">
              <button
                type="button"
                role="radio"
                aria-checked={scheme === "quanpin"}
                className={onboarding.schemeOption(scheme === "quanpin")}
                onClick={() => setScheme("quanpin")}
              >
                <span aria-hidden="true">⌨</span>
                <span>
                  <strong>全拼 26 键</strong>
                  <small>完整字母，熟悉的输入手感</small>
                </span>
                <b aria-hidden="true">{scheme === "quanpin" ? "✓" : "○"}</b>
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={scheme === "nine_key"}
                className={onboarding.schemeOption(scheme === "nine_key")}
                onClick={() => setScheme("nine_key")}
              >
                <span aria-hidden="true">▦</span>
                <span>
                  <strong>全拼 9 键</strong>
                  <small>大按键，单手输入更方便</small>
                </span>
                <b aria-hidden="true">{scheme === "nine_key" ? "✓" : "○"}</b>
              </button>
            </div>
          </section>
        )}
        {page === 2 && (
          <section className={onboarding.section}>
            <h2 className={onboarding.sectionTitle}>打字时顺便看懂英文</h2>
            <p className={onboarding.lead}>
              在候选词下方标出它的英文意思。释义来自随键盘打包的离线词库，不联网。
            </p>
            <div className={onboarding.glossPreview} aria-hidden="true">
              {glossSamples.map(([word, meaning], index) => (
                <div className={onboarding.glossCandidate(index === 0)} key={word}>
                  <span>{word}</span>
                  {gloss && <small>{meaning}</small>}
                </div>
              ))}
            </div>
            <div className={onboarding.glossRow}>
              <span>
                <strong>显示英文释义</strong>
                <small>稍后可以在「标点与翻译」中修改。</small>
              </span>
              <Switch aria-label="显示英文释义" checked={gloss ?? false} onChange={setGloss} />
            </div>
          </section>
        )}
        {page === 3 && (
          <section className={onboarding.section}>
            <h2 className={onboarding.sectionTitle}>一个账号，多台设备</h2>
            <p className={onboarding.lead}>
              登录后可以把词库保存到账号，让设置和皮肤跟着账号走，并在设备之间共用云剪贴板。
            </p>
            {/* The design's three perks (dc.html `onbPerks`), less 自造词: no host uploads the user's own words. */}
            <ul className={onboarding.perks}>
              {perks.map(([icon, label]) => (
                <li className={onboarding.perk} key={label}>
                  <span aria-hidden="true">{icon}</span>
                  {label}
                </li>
              ))}
            </ul>
            <p className={onboarding.note}>
              日常输入无需登录，默认保持离线；不登录也可以直接开始使用。
            </p>
          </section>
        )}
      </div>
      {error && (
        <p className={onboarding.error} role="alert">
          {error}
        </p>
      )}
      {footer}
    </>
  );

  const modal = ipad || desktop;
  return (
    // Marked the same way the settings shell is: this page owns the whole window and renders before the shell exists, so without its own marker the first screen anyone sees is the only one still in the desktop accent.
    <main
      className={`${modal ? onboarding.sheetBackdrop : onboarding.page} ${onboarding.buttons}`}
      {...shell}
    >
      {modal ? (
        <>
          {/* The dimmed backdrop, which a tap dismisses the way 跳过 does (dc.html `onbModal`). A pointer-only affordance: the same action is a button inside the sheet. */}
          <div className={onboarding.sheetScrim} aria-hidden="true" onClick={skipFlow} />
          <div className={onboarding.sheet(desktop)}>{content}</div>
        </>
      ) : (
        content
      )}
    </main>
  );
}
