import { useCallback, useEffect, useRef, useState, type PointerEvent } from "react";
import { Switch } from "../core/platform-controls";
import { ActionButton } from "../core/action-button";
import { useAsyncActionRunner } from "../core/use-async-action";
import { FluentIcon, type FluentIconName } from "../core/fluent-icons";
import { msimeFramePath, msimeStrokePath } from "../core/brand-logo";
import { appThemeStyle, seasonAttr } from "../core/app-theme-style";
import type { HostChromeClient, ImeSetupClient, ResolvedAppTheme } from "../core/host-contracts";
import { makeDefaultAction, useImeSetupState } from "../keyboard/setup-status-card";
import { useSettingsPlatform } from "../theme/settings-platform";
import { useHarmonyWelcomeSystemBars } from "../settings/use-settings-theme";
import type { EditionInfo, InputScheme, Preferences } from "../index";
import * as onboarding from "./onboarding-style";

/** 首次设置选用的触屏键盘。全拼 26 键和 9 键由用户在「选择输入方式」一步里选，HarmonyOS 还可以选双拼（小鹤）和五笔；不提供全拼的版本跳过这一步，直接用版本默认方案的键盘（五笔版是五笔）。 */
export type OnboardingInputScheme = "quanpin" | "nine_key" | "xiaohe" | "wubi";

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
  /** 用户在选定键盘之前（既没点过方案，也没走过方案那一步）就点了跳过时为 true：偏好保留原有的方案，不采用流程预选的那个。 */
  keepScheme?: boolean;
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

/** HarmonyOS 启动页（dc.html L2692-2698）：白色圆盘落到强调色底上并泛起波纹，标志旋转入场并描出自身线条，底部一条进度条在启动页停留期间填满。点一下仍可跳过；设计里没有提示这一点。 */
function HarmonySplash({ onDone }: { onDone: () => void }) {
  useEffect(() => {
    const timer = window.setTimeout(onDone, SPLASH_MS);
    return () => window.clearTimeout(timer);
  }, [onDone]);
  return (
    <button type="button" className={onboarding.hSplash} aria-label="跳过开屏" onClick={onDone}>
      <span className={onboarding.hSplashGlow} aria-hidden="true" />
      <span className={onboarding.hSplashDiscBox} aria-hidden="true">
        <span className={onboarding.hSplashRipple} />
        <span className={onboarding.hSplashDisc} />
        <svg className={onboarding.hSplashLogo} viewBox="0 0 116 132">
          <path className={onboarding.hSplashFrame} d={msimeFramePath} />
          <path className={onboarding.hSplashStroke} d={msimeStrokePath} pathLength={1} />
        </svg>
      </span>
      <span className={onboarding.splashName} aria-hidden="true">
        水杉输入法
      </span>
      <span className={onboarding.splashTagline} aria-hidden="true">
        METASEQUOIA IME
      </span>
      <span className={onboarding.hSplashTrack} aria-hidden="true">
        <span className={onboarding.hSplashFill} />
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

// ---- HarmonyOS 流程步骤（dc.html `flowVals` STEPS） ----

/** 每一步的 Fluent 图标，按步骤序号排列。 */
const harmonyStepIcons: readonly FluentIconName[] = [
  "keyboard",
  "text_font",
  "translate",
  "arrow_sync_circle",
];
/** 每一步的眉题在序号之后的文字，按步骤序号排列；最后一步的眉题只有「最后一步」。 */
const harmonyKickerTails = ["约 30 秒", "随时可以改", "水杉的特点"] as const;
const harmonyOrdinals = ["第一步", "第二步", "第三步"] as const;
const harmonyStepTitles = [
  "把水杉加进键盘",
  "选一套输入方案",
  "候选下方就是译文",
  "登录后多端同步",
] as const;

/** 一张方案卡片：它选用的键盘、版本须为它提供的引擎方案、它在该步正文里的名称，以及它的两行文字。 */
interface HarmonySchemeCard {
  scheme: OnboardingInputScheme;
  input: InputScheme;
  name: string;
  label: string;
  detail: string;
}

/** 设计里的四张卡片，去掉宿主或当前版本不提供的，2in1 上再去掉 9 键，因为它有硬件键盘（dc.html `onbSchemes`）。五笔那一行写明正在用哪套码表，与 Android 一致。 */
function harmonySchemeCards(
  desktop: boolean,
  edition: EditionInfo | undefined,
  inputSchemes: readonly InputScheme[] | undefined,
  wubiProfile: Preferences["wubi_profile"],
): HarmonySchemeCard[] {
  const all: HarmonySchemeCard[] = [
    {
      scheme: "quanpin",
      input: "quanpin",
      name: "全拼",
      label: "全拼 26 键",
      detail: "最常用，完整拼音",
    },
    {
      scheme: "nine_key",
      input: "quanpin",
      name: "9 键",
      label: "全拼 9 键",
      detail: "单手更顺手",
    },
    {
      scheme: "xiaohe",
      input: "shuangpin",
      name: "双拼",
      label: "双拼",
      detail: "每字两键 · 默认小鹤",
    },
    {
      scheme: "wubi",
      input: "wubi",
      name: "五笔",
      label: "五笔",
      detail: wubiProfile === "wubi98" ? "形码 · 当前 98 版" : "形码 · 默认 86 版",
    },
  ];
  return all.filter(
    (card) =>
      !(desktop && card.scheme === "nine_key") &&
      (!edition || edition.input_schemes.includes(card.input)) &&
      (!inputSchemes || inputSchemes.includes(card.input)),
  );
}

/** 方案那一步的正文只列出实际提供的卡片：「全拼、9 键、双拼和五笔用的是…」。 */
function harmonySchemeLead(cards: readonly HarmonySchemeCard[]): string {
  const names = cards.map((card) => card.name);
  const listed =
    names.length <= 1
      ? (names[0] ?? "")
      : `${names.slice(0, -1).join("、")}和${names[names.length - 1]}`;
  return `${listed}用的是同一套引擎，词库和自造词通用。`;
}

/** 设计里的候选栏（dc.html `onbCands`）：一个音节的候选及其英文释义。 */
const harmonyGlossSamples = [
  ["候选", "candidate"],
  ["后选", "choice"],
  ["侯选", "option"],
  ["候", "wait"],
] as const;

/** 登录带来的好处，配设计里的图标；不列自造词，因为没有宿主上传用户自己的词。 */
const harmonyPerks: readonly (readonly [FluentIconName, string])[] = [
  ["book", "词库"],
  ["color", "皮肤与主题"],
  ["clipboard", "云剪贴板"],
];

/** HarmonyOS 键盘自身的背景色（`GlobalTheme` 的浅色与深色），第 3 步的候选栏借用它，看起来就是键盘。有季节主题时改用由强调色调出的颜色，与键盘在「跟随系统」下的做法一致（`AppThemePalette.keyboard`，dc.html `kbBase.bg`）：浅色模式叠在页面色上，深色模式叠在键盘自己的中性色 #161716 上，因为季节的页面色会让候选栏融进页面。 */
function harmonyStripColors(
  seasonal: boolean,
  dark: boolean,
): { background: string; gloss: string } {
  return {
    background: seasonal
      ? dark
        ? "color-mix(in srgb, var(--accent-color) 10%, #161716)"
        : "color-mix(in srgb, var(--accent-color) 12%, var(--p-bg))"
      : dark
        ? "#121814"
        : "#D9E2D6",
    gloss: dark ? "#93A596" : "#5A6B5D",
  };
}

/** HarmonyOS 手机每一步压入的一条历史记录，让系统返回手势（即 WebView 后退）在流程里逐步后退。 */
interface OnboardingHistoryEntry {
  page: number;
  /** 栈上属于流程的记录数，截至并包含这一条。 */
  depth: number;
}

function onboardingHistoryEntry(state: unknown): OnboardingHistoryEntry | undefined {
  if (!state || typeof state !== "object" || !("msimeOnboarding" in state)) return undefined;
  const entry = (state as { msimeOnboarding: unknown }).msimeOnboarding;
  if (!entry || typeof entry !== "object") return undefined;
  const { page, depth } = entry as Partial<OnboardingHistoryEntry>;
  return typeof page === "number" && typeof depth === "number" ? { page, depth } : undefined;
}

/** 设计里算作翻页的横向滑动：横向至少 50px，且横向距离至少是纵向的 1.5 倍（dc.html `onbSw`）。 */
const SWIPE_MIN_PX = 50;
const SWIPE_RATIO = 1.5;

/** 主题 hook 最近一次写入的文档外观；它变化时宿主会重新渲染流程。 */
function documentDark(): boolean {
  return typeof document !== "undefined" && document.documentElement.dataset.theme === "dark";
}

export function WelcomeFlowPage({
  actions,
  onComplete,
  onSkip,
  splash = false,
  edition,
  setup,
  signedIn,
  appTheme,
  inputSchemes,
  wubiProfile,
  chrome,
}: {
  actions: OnboardingActions;
  onComplete: (scheme: OnboardingInputScheme, choices: OnboardingChoices) => Promise<void>;
  /** 提前离开流程，参数与 `onComplete` 相同：跳过会保留流程里已选的键盘（见 `OnboardingChoices.keepScheme`）和已切换的释义开关。 */
  onSkip?: (scheme: OnboardingInputScheme, choices: OnboardingChoices) => Promise<void>;
  /** Opens on the splash. Only a first launch passes it; a replay from settings goes straight to the steps. */
  splash?: boolean;
  /** 运行中的版本（`HostCapabilities.edition`），不是 full 时才有。不提供全拼的版本没有「选择输入方式」这一步。 */
  edition?: EditionInfo;
  /** HarmonyOS：宿主的启用状态，有了它第一步就是设计里的两项实时检查。没有时这一步保留编号说明。 */
  setup?: ImeSetupClient;
  /** 是否已登录账号：已登录时最后一步以「开始使用」结束，不再提供「登录」和「稍后再说」。宿主仍在查询时为 undefined，按未登录处理。 */
  signedIn?: boolean;
  /** HarmonyOS：按当前外观解析出的用户应用主题，像给设置页换色一样给流程换色。 */
  appTheme?: ResolvedAppTheme | null;
  /** HarmonyOS：宿主提供的方案（`HostCapabilities.input_schemes`）；方案缺失的键盘不提供。 */
  inputSchemes?: readonly InputScheme[];
  /** HarmonyOS：已保存的五笔码表，让五笔卡片写明正在用哪一套。 */
  wubiProfile?: Preferences["wubi_profile"];
  /** HarmonyOS 手机：宿主的系统栏，先按启动页着色，再按各步骤的页面着色。 */
  chrome?: HostChromeClient;
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
  // 用户是否已选定键盘：点过某个键盘，或走过了提供键盘的那一步；在此之前点跳过不动已保存的方案。
  const schemeChosen = useRef(false);
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
  // HarmonyOS 手机：设计里的页面，后退只能靠滑动和系统返回手势。
  const harmonyPhone = platform === "harmony";
  const [splashing, setSplashing] = useState(splash && !desktop);
  const endSplash = useCallback(() => setSplashing(false), []);
  const setupState = useImeSetupState(harmony ? setup : undefined);
  const schemeCards = harmony
    ? harmonySchemeCards(desktop, edition, inputSchemes, wubiProfile)
    : [];
  // HarmonyOS 上没有全拼卡片时，预选的键盘是提供的第一张卡片。
  const chosenScheme =
    editionScheme ??
    (!harmony || schemeCards.length === 0 || schemeCards.some((card) => card.scheme === scheme)
      ? scheme
      : schemeCards[0].scheme);

  // HarmonyOS 手机每前进一步压入一条历史记录，让系统返回手势（它使 WebView 后退）在流程里逐步后退；流程交接之前再把这些记录撤掉。
  const pushedEntries = useRef(0);
  const rewinding = useRef(false);
  useEffect(() => {
    if (!harmonyPhone) return;
    const onPopState = (event: PopStateEvent) => {
      if (rewinding.current) return;
      const entry = onboardingHistoryEntry(event.state);
      pushedEntries.current = entry?.depth ?? 0;
      setPage(entry?.page ?? 0);
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, [harmonyPhone]);
  const swipeStart = useRef<{ x: number; y: number } | null>(null);

  const goTo = (next: number) => {
    setPage(next);
    if (!harmonyPhone) return;
    pushedEntries.current += 1;
    const entry: OnboardingHistoryEntry = { page: next, depth: pushedEntries.current };
    window.history.pushState({ msimeOnboarding: entry }, "");
  };
  const goBack = () => {
    if (harmonyPhone && pushedEntries.current > 0) window.history.back();
    else setPage(steps[step - 1] ?? 0);
  };
  // 把流程自己的记录从栈上撤掉，等 WebView 回到流程打开时的那条记录再 resolve，这样后续页面从干净的历史开始，不会收到本该给流程的迟到 popstate。
  const rewindHistory = () => {
    const depth = pushedEntries.current;
    if (!harmonyPhone || depth === 0) return Promise.resolve();
    return new Promise<void>((resolve) => {
      rewinding.current = true;
      const landed = () => {
        window.removeEventListener("popstate", landed);
        pushedEntries.current = 0;
        rewinding.current = false;
        resolve();
      };
      window.addEventListener("popstate", landed);
      window.history.go(-depth);
    });
  };

  const run = async (operation: () => Promise<void>, next?: number) => {
    if (busy) return;
    await runAsyncAction(
      async (isCurrent) => {
        await operation();
        if (isCurrent() && next !== undefined) goTo(next);
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
    void run(async () => {
      await rewindHistory();
      await onComplete(chosenScheme, { candidateEnglishGloss: gloss, openAccount });
    });
  // Skipping leaves the walkthrough, not the preparation: the keyboard still needs its dictionaries, and on Android the flow keeps coming back until they are in place.
  const skipFlow = onSkip
    ? () =>
        void run(async () => {
          await ensureResources();
          await rewindHistory();
          await onSkip(chosenScheme, {
            candidateEnglishGloss: gloss,
            openAccount: false,
            ...(schemeChosen.current ? {} : { keepScheme: true }),
          });
        })
    : undefined;

  const advance = () => {
    if (page === 1) schemeChosen.current = true;
    // 准备之后才可能知道版本，下一步按准备之后的步骤定（见 `page`）。
    if (page === 0) void run(ensureResources, 1);
    // 已登录时，最后一步的操作是「开始使用」，没有账号页可打开。
    else if (page === 3) finish(!signedIn);
    else goTo(steps[step + 1] ?? 3);
  };
  // The last step's action is signing in (dc.html `STEPS[3].cta`); leaving without it is the 稍后再说 next to it.
  const nextLabel = busy ? "正在准备…" : page === 3 ? (signedIn ? "开始使用" : "登录") : "下一步";

  const dark = documentDark();
  useHarmonyWelcomeSystemBars(harmonyPhone ? chrome : undefined, appTheme ?? null, dark, splashing);
  const shell = {
    "aria-label": "首次设置",
    "data-onboarding-shell": "",
    "data-mobile": desktop ? undefined : "",
    "data-platform": platform,
    // HarmonyOS 在承载平台 token 的同一元素上应用用户的应用主题，与设置根节点相同。
    "data-season": harmony ? seasonAttr(appTheme) : undefined,
    style: harmony ? appThemeStyle(appTheme ?? null, dark, desktop ? "hm2" : "harmony") : undefined,
  };

  if (splashing)
    return (
      // Marked the same way as the flow, so the splash takes the brand palette rather than the desktop one.
      <main className={harmony ? onboarding.hSplashShell : onboarding.splashShell} {...shell}>
        {harmony ? <HarmonySplash onDone={endSplash} /> : <Splash onDone={endSplash} />}
      </main>
    );

  if (harmony) {
    const stepContent =
      page === 0 ? (
        setup ? (
          <div className={onboarding.hChecks(desktop)}>
            {[
              {
                label: desktop ? "已在「系统 → 输入法」中启用" : "已在系统中启用",
                done: setupState?.enabled ?? null,
                actionLabel: "去开启",
                action: actions.openSystemKeyboardSettings,
              },
              {
                label: "设为默认输入法",
                done: setupState?.current ?? null,
                actionLabel: "设为默认",
                action: makeDefaultAction(actions, setupState),
              },
            ].map(({ label, done, actionLabel, action }, index) => (
              <div className={onboarding.hCheck(index > 0)} key={label}>
                <span className={onboarding.hCheckBadge(done)} aria-hidden="true">
                  {done === true ? "✓" : done === false ? "!" : ""}
                </span>
                <span className={onboarding.hCheckLabel}>
                  {label}
                  <span className="sr-only">
                    {done === true ? "，已完成" : done === false ? "，未完成" : "，正在检查"}
                  </span>
                </span>
                {/* 修复某项检查的操作，只在该项未完成时显示：完成后无需操作，宿主尚未回报时也无从提供。 */}
                {done === false && action && (
                  <ActionButton
                    action={() => void run(withResources(action))}
                    className={onboarding.hCheckAction}
                    disabled={busy}
                    label={actionLabel}
                  />
                )}
              </div>
            ))}
          </div>
        ) : (
          <>
            <div className={onboarding.hSetupCard(desktop)}>
              <SetupStep number={1} title="打开键盘设置">
                前往 HarmonyOS 的系统输入法设置。
              </SetupStep>
              <SetupStep number={2} title="启用水杉输入法">
                在可用输入法列表中打开水杉输入法。
              </SetupStep>
              <SetupStep number={3} title="切换并开始输入" last>
                在输入框中选择水杉输入法即可开始使用。
              </SetupStep>
            </div>
            <div className={onboarding.hSystemActions}>
              <ActionButton
                action={() => void run(withResources(actions.openSystemKeyboardSettings))}
                className="primary"
                disabled={busy}
                label="打开系统设置"
              />
              <ActionButton
                action={() => void run(withResources(actions.showInputMethodPicker))}
                className="secondary"
                disabled={busy}
                label="选择输入法"
              />
            </div>
            <p className={onboarding.hNote}>
              系统设置页面由 HarmonyOS 管理，水杉不会自动启用或切换输入法。
            </p>
          </>
        )
      ) : page === 1 ? (
        <div className={onboarding.hSchemes} role="radiogroup" aria-label="首次输入方案">
          {schemeCards.map((card) => {
            const selected = card.scheme === chosenScheme;
            return (
              <button
                type="button"
                role="radio"
                aria-checked={selected}
                key={card.scheme}
                className={onboarding.hScheme(desktop, selected)}
                onClick={() => {
                  schemeChosen.current = true;
                  setScheme(card.scheme);
                }}
              >
                <span className={onboarding.hSchemeText}>
                  <strong>{card.label}</strong>
                  <span>{card.detail}</span>
                </span>
                <span
                  className={onboarding.hRadio(selected)}
                  // 选中单选框的圆心用页面色，与强调色圆环相对：浅色下为白色，深色下为黑色（dc.html `o.dotBg`）。
                  style={{ background: selected ? (dark ? "#000000" : "#FFFFFF") : "transparent" }}
                  aria-hidden="true"
                />
              </button>
            );
          })}
        </div>
      ) : page === 2 ? (
        <>
          {/* 候选栏的静态示意，衬在键盘自身的背景色上；下方开关打开时显示释义。 */}
          <div
            className={onboarding.hGlossStrip(desktop)}
            style={{ background: harmonyStripColors(Boolean(appTheme), dark).background }}
            aria-hidden="true"
          >
            {harmonyGlossSamples.map(([word, meaning], index) => (
              <div className={onboarding.hGlossCell(index === 0)} key={word}>
                <span>{word}</span>
                {gloss && (
                  <small style={{ color: harmonyStripColors(Boolean(appTheme), dark).gloss }}>
                    {meaning}
                  </small>
                )}
              </div>
            ))}
          </div>
          <div className={onboarding.hGlossRow(desktop)}>
            <span>显示英文释义</span>
            <Switch aria-label="显示英文释义" checked={gloss ?? false} onChange={setGloss} />
          </div>
        </>
      ) : (
        <ul className={onboarding.hPerks}>
          {harmonyPerks.map(([icon, label]) => (
            <li className={onboarding.hPerk(desktop)} key={label}>
              <span className={onboarding.hPerkTile} aria-hidden="true">
                <FluentIcon name={icon} size={18} />
              </span>
              {label}
            </li>
          ))}
        </ul>
      );
    const lead =
      page === 0
        ? desktop
          ? "在 设置 → 系统 → 输入法 里启用水杉，并设为默认输入法。"
          : "在系统设置里启用水杉，并设为默认输入法，之后在任何应用里都能直接用。"
        : page === 1
          ? harmonySchemeLead(schemeCards)
          : page === 2
            ? // 设计说释义会回退到联网查询；实际只来自随键盘打包的词库。
              "打开后，每个候选词下面会多一行小字的英文释义，来自随键盘打包的离线词库，不联网。"
            : // 设计还列了自造词；没有宿主上传用户自己的词。
              "登录后词库、皮肤和云剪贴板会在手机、平板和电脑之间同步。";
    const body = (
      <div className={onboarding.hBody(desktop)}>
        <span className={onboarding.hIconTile(desktop)} aria-hidden="true">
          <FluentIcon name={harmonyStepIcons[page]} size={36} />
        </span>
        <span className={onboarding.hKicker}>
          {page === 3 ? "最后一步" : `${harmonyOrdinals[step]} · ${harmonyKickerTails[page]}`}
        </span>
        <h1 className={onboarding.hTitle(desktop)}>
          {page === 0 && desktop ? "设为系统输入法" : harmonyStepTitles[page]}
        </h1>
        <p className={onboarding.hLead}>{lead}</p>
        <div className={onboarding.hContent}>{stepContent}</div>
      </div>
    );
    const errorLine = error && (
      <p className={onboarding.hError} role="alert">
        {error}
      </p>
    );
    const later = page === 3 && !signedIn && (
      <ActionButton
        action={() => finish(false)}
        className={desktop ? onboarding.hDeskLink : onboarding.hLater}
        disabled={busy}
        label="稍后再说"
      />
    );

    if (desktop)
      return (
        <main className={`${onboarding.hSheetBackdrop} ${onboarding.buttons}`} {...shell}>
          {/* 变暗的背景，点一下与跳过一样关闭流程（dc.html `onbModal`）。仅供指针操作：同样的操作在面板内有对应按钮。 */}
          <div className={onboarding.sheetScrim} aria-hidden="true" onClick={skipFlow} />
          <div className={onboarding.hSheet}>
            <p className={onboarding.hDeskCounter}>{`第 ${step + 1} 步，共 ${steps.length} 步`}</p>
            {body}
            {errorLine}
            {/* 桌面版底栏（dc.html `onbDeskBar`）：每一步左侧都是跳过，右侧是上一步和主操作。 */}
            <footer className={onboarding.hDeskBar}>
              {skipFlow ? (
                <ActionButton
                  action={skipFlow}
                  className={onboarding.hDeskLink}
                  disabled={busy}
                  label="跳过"
                />
              ) : (
                later
              )}
              <span className={onboarding.deskSpacer} />
              {page > 0 && (
                <ActionButton
                  action={goBack}
                  className={onboarding.hDeskBack(dark)}
                  disabled={busy}
                  label="上一步"
                />
              )}
              <ActionButton
                action={advance}
                className={onboarding.hDeskNext}
                disabled={busy}
                label={nextLabel}
              />
            </footer>
          </div>
        </main>
      );

    const onSwipeStart = (event: PointerEvent<HTMLElement>) => {
      swipeStart.current = { x: event.clientX, y: event.clientY };
    };
    // 左滑在最后一步之前相当于下一步，右滑后退一步；手机上没有上一步按钮（dc.html `onbSw`）。
    const onSwipeEnd = (event: PointerEvent<HTMLElement>) => {
      const start = swipeStart.current;
      swipeStart.current = null;
      if (!start || busy) return;
      const dx = event.clientX - start.x;
      const dy = event.clientY - start.y;
      if (Math.abs(dx) < SWIPE_MIN_PX || Math.abs(dx) < SWIPE_RATIO * Math.abs(dy)) return;
      if (dx < 0) {
        if (step < steps.length - 1) advance();
      } else if (step > 0) goBack();
    };
    return (
      <main
        className={`${onboarding.hPage} ${onboarding.buttons}`}
        {...shell}
        onPointerDown={onSwipeStart}
        onPointerUp={onSwipeEnd}
        onPointerCancel={() => {
          swipeStart.current = null;
        }}
      >
        <div className={onboarding.hTopRow}>
          <span className={onboarding.hCounter}>{`${step + 1} / ${steps.length}`}</span>
          {skipFlow && (
            <ActionButton
              action={skipFlow}
              className={onboarding.hSkip}
              disabled={busy}
              label="跳过"
            />
          )}
        </div>
        {body}
        {errorLine}
        <footer className={onboarding.hFooter}>
          <div
            className={onboarding.hDots}
            role="img"
            aria-label={`第 ${step + 1} 步，共 ${steps.length} 步`}
          >
            {steps.map((index) => (
              <i key={index} className={onboarding.hDot(index === page)} />
            ))}
          </div>
          <ActionButton
            action={advance}
            className={onboarding.hCta}
            disabled={busy}
            label={nextLabel}
          />
          {later}
        </footer>
      </main>
    );
  }

  const skip = skipFlow && (
    <ActionButton action={skipFlow} className={onboarding.skip} disabled={busy} label="跳过" />
  );
  const later = (
    <ActionButton
      action={() => finish(false)}
      className={android ? onboarding.textButton : onboarding.later}
      disabled={busy}
      label="稍后再说"
    />
  );
  const back = (
    <ActionButton
      action={goBack}
      className={android ? onboarding.textButton : onboarding.back}
      disabled={busy}
      label="上一步"
    />
  );
  const next = (
    <ActionButton
      action={advance}
      className={android ? "primary" : `primary ${onboarding.next}`}
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
          {/* Android 用上方的进度条显示进度。 */}
          {!android && <p className={onboarding.progress}>{`${step + 1} / ${steps.length}`}</p>}
          <h1>{stepTitles[page]}</h1>
        </div>
        {/* 手机和 iPad 把跳过放在顶部（dc.html `ob.topSkip`）；Android 放在按钮栏里。 */}
        {!android && skip}
      </header>
      <div className={onboarding.body}>
        {page === 0 && (
          <section className={onboarding.section}>
            <h2 className={onboarding.sectionTitle}>{ios ? "添加水杉键盘" : "添加水杉输入法"}</h2>
            <p className={onboarding.lead}>
              {ios
                ? "在系统键盘列表中启用水杉，再回到任意输入框开始使用。"
                : "准备好内置词库后，按下面步骤启用系统键盘。"}
            </p>
            <div className={onboarding.setupCard}>
              <SetupStep number={1} title="打开键盘设置">
                {ios
                  ? "前往系统设置中的“通用 → 键盘 → 键盘”。"
                  : "前往系统设置中的“语言和输入法”或“屏幕键盘”。"}
              </SetupStep>
              <SetupStep number={2} title={ios ? "添加水杉键盘" : "启用水杉输入法"}>
                {ios ? "在第三方键盘列表中添加水杉键盘。" : "在可用输入法列表中打开水杉输入法。"}
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
                onClick={() => {
                  schemeChosen.current = true;
                  setScheme("quanpin");
                }}
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
                onClick={() => {
                  schemeChosen.current = true;
                  setScheme("nine_key");
                }}
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
