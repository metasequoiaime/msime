import { StrictMode } from "react";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  SettingsPage,
  SettingsStartupPage,
  WelcomeFlowPage,
  type DictionaryClient,
  type DictionaryCollectionsClient,
  type DictionaryCollectionsView,
  type DictionaryEntry,
  type DictionaryManifest,
  type DictionaryImportResult,
  type HostCapabilities,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
  type Preferences,
  type SavedTouchKeyboardSkin,
  type SkinCatalog,
  type SkinFont,
  type SkinImage,
  CloudClipboardPanel,
  CloudDictionaryPanel,
  CloudDictionaryCatalogPanel,
  CloudDictionaryFilesPanel,
  CloudDictionaryApplyPanel,
  CloudCandidatesPanel,
  completeOnboardingPreferences,
  resolveSettingsTheme,
  settingsThemePreferences,
  useSettingsTheme,
  type AccountClient,
  type AccountPreferences,
  type AiSkinClient,
  type AiSkinProposal,
  type CommunityResource,
  type CommunityResourceApplication,
  type CommunityResourceClient,
  type CommunityResourcePage,
  type CommunitySkin,
  type CommunitySkinClient,
  type CommunitySkinDownload,
  type CommunitySkinPage,
  type AccountPreferenceSchema,
  type SettingsSyncClient,
  type ChatClient,
  type ChatModels,
  type CloudClipboardPanelClient,
  type CloudDictionaryAction,
  type CloudDictionaryPanelClient,
  type SettingsClient,
  type Snapshot,
  type UpdateCheckRequest,
  type UpdateCheckResult,
  type StatisticsRetention,
  type TypingStatisticsClient,
  type VocabularyReviewClient,
  type VocabularyReviewStatus,
  type TypingStatisticsStatus,
  type LocalVoiceModelClient,
  type LocalVoiceModelList,
  type LocalVoiceModelProgress,
  type MentionEntry,
  type PluginCatalogResult,
  type PluginClient,
  type PluginPackage,
  UNBATCHED_DICTIONARY_FILE_BYTES,
} from "@msime/ui";
import type {
  AiAssistantClient,
  ApiCredentialTestResult,
  ApiCredentialTestService,
  AppThemeCatalogEntry,
  AppThemeClient,
  AppThemeId,
  FeedbackClient,
  HostChromeClient,
  ImeSetupClient,
  ImeSetupState,
  MobileKeyboardFeedback,
  OnboardingChoices,
  OnboardingInputScheme,
  ResolvedAppTheme,
  SurfaceTheme,
  ThemeMode,
  VoiceCaptureDevice,
} from "@msime/ui";
import "@msime/ui/styles.css";

/**
 * The settings UI, hosted by the HarmonyOS application.
 *
 * Everything the page needs from the system arrives through one object the ArkTS side injects into
 * the Web component. It is deliberately thin: JSON in, JSON out, the same shape the shared C ABI
 * uses, so the bridge has no opinion about preferences and nothing to keep in sync with the schema.
 *
 * Only `load` and `save` are required of a SettingsClient. Everything else on the interface is a
 * capability some host happens to have, and the page renders what the host says it can do rather
 * than what the platform is called, so an unimplemented surface simply does not appear.
 */

interface NativeBridge {
  /** `{"ok":true,"value":{"revision":n,"preferences":{...}}}` or `{"ok":false,"error":"..."}`. */
  loadPreferences(): string;
  savePreferences(expectedRevision: number, document: string): string;
  /** The capability record for this host, as client-core writes it. */
  hostCapabilities(): string;
  scanSkinCatalog(): string;
  readSkinImage(id: string, relative: string): string;
  readSkinFont(id: string, relative: string): string;
  readSkinToolbarCss(id: string): string;
  /** `{profile,sourceCommit}` from the packaged dictionary manifest, or a refusal. */
  dictionaryManifest(): string;
  appVersion(): string;
  dictionary(action: string): string;
  cloudDictionaryDownload(entry: string): string;
  typingStatistics(action: string): string;
  /**
   * Starts one of the asynchronous requests and returns at once.
   *
   * A bridge method that returns a Promise never settles on this platform, so the account, the
   * account-backed chat, the cloud dictionary and its snapshots, the AI model list, the AI test and
   * the credential test are started by number and answered later through `msimeHarmonyBridgeReply`.
   *
   * The caller chooses the deadline, because the kinds do not share one: a completion is a model
   * writing text and is given the same 125 seconds the shared clients allow it, while everything
   * else answers from a database and should report a stalled network in seconds.
   */
  startRequest(kind: string, id: number, payload: string): string;
  openExternalUrl(url: string): void;
  copyText(text: string): void;
  openSystemKeyboardSettings(): void;
  /** `{"ok":true,"value":[{backend,id,label},...]}`; a refusal is reported, not an empty list. */
  listVoiceCaptureDevices(): string;
  /** `{"ok":true,"value":["Family",...]}`; a refusal is reported so the page can say so. */
  listFontFamilies(): string;
  /** `{operation:"load"|"save"|"preview",...}`; the reply carries the settings now in force. */
  keyboardFeedback(request: string): string;
  /**
   * Whether setup still has a step left: not enabled, or enabled but not the current keyboard.
   *
   * Synchronous, and the reply says whether the answer is ready yet — a bridge method that returns
   * a Promise never settles on this platform, so the host prepares the answer instead and the page
   * waits for `ready` rather than acting on a default.
   */
  onboardingStatus(): string;
  /** The system's keyboard picker, which is where the second setup step happens. */
  showInputMethodPicker(): string;
  /** Starts the picker and answers at once; the panel's own rescan is what shows the result. */
  importSkinFolder(): string;
  /**
   * The 插件 page's pack store and @ name list: `{operation:"catalog"|"remove"|"load_mentions"|"save_mentions",...}`, answered by `msime_client_plugins` as `{ok,value}` or `{ok:false,error,detail?}`. An import waits for the system picker, so it goes through `startRequest` as `plugin_import` instead.
   */
  plugins(action: string): string;
  // 下面这些方法随 HarmonyOS 改版加入。旧版 HAP 没有注册它们，所以每个都先用 `typeof` 检查再用，缺哪个，页面就不画它支撑的那块界面，而不是去调用不存在的东西。它们声明为普通成员，这样桥接一致性检查（`scripts/test-harmony-bridge-parity.py`）仍能看到它们，并要求宿主把每一个都注册上。
  /**
   * `{"ok":true,"ready":boolean,"value":{"enabled":boolean|null,"current":boolean|null}}`：本键盘是否已在系统中启用、是否为当前输入法。和 `onboardingStatus` 一样是同步调用，宿主第一次读取完成前 `ready` 一直为 false。
   */
  setupStatus(): string;
  /**
   * 应用主题：`{operation:"load"}` 和 `{operation:"save",id}` 回复 `{ok,value:{id}}`；`{operation:"resolve",dark}` 回复 `msime_client_resolve_app_theme` 对已保存主题在本月的解析结果；`{operation:"catalog"}` 回复 `msime_client_app_theme_catalog` 的结果。
   */
  appTheme(request: string): string;
  /** `{background,navigationBar,dark}`，均为 `#RRGGBB` 颜色：宿主据此把状态栏和导航栏涂成与页面一致。 */
  setSystemBars(bars: string): void;
  /** `{"ok":true,"value":boolean}`：欢迎闪屏是否已在本设备上播放过。 */
  splashSeen(): string;
  /** 记录闪屏已播放。写在宿主的文件里而不是页面存储里，因为从资源加载的页面不保证留得住页面存储。 */
  markSplashSeen(): void;
  /** 从设置重放欢迎流程，宿主在流程显示期间暂缓显示通知卡片。 */
  onboardingStarted(): void;
  /** 欢迎流程已结束（走完或跳过），宿主可以重新显示它的通知卡片。 */
  onboardingFinished(): void;
  /** `{"ok":true,"value":string|null}`：键盘要求本窗口打开的页面，只交出一次，第二次读取回复 null。 */
  pendingPage(): string;
}

declare global {
  // eslint-disable-next-line no-var
  var msimeHarmonyPreferencesChanged: ((reply: string) => void) | undefined;
  // 宿主把 `{enabled,current}` 值本身作为脚本字面量传进来。
  // eslint-disable-next-line no-var
  var msimeHarmonySetupChanged: ((state: unknown) => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyAppThemeChanged: (() => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyOpenPage: ((page: string) => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyBridgeReply: ((id: number, reply: string) => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyAiSkinProgress: ((requestId: string, completed: number) => void) | undefined;
  // eslint-disable-next-line no-var
  var msimeHarmonyVoiceModelProgress: ((document: string) => void) | undefined;
}

/**
 * The asynchronous half of the bridge.
 *
 * The host cannot answer through a returned Promise on this platform, so a request is started by
 * number and the answer arrives later on a global the host calls. Each request keeps its own
 * resolver until then.
 *
 * Numbers are per page load and never reused: a reply for a number nobody is waiting on belongs to
 * a request that timed out or was made before a reload, and delivering it to whoever holds that
 * number now would answer the wrong question.
 *
 * Requests do time out. A host that never replies would otherwise leave a settings control
 * spinning with nothing to cancel it, and a reported failure is something the page can show.
 */
const pendingBridgeRequests = new Map<number, (reply: string) => void>();
let nextBridgeRequestId = 1;

globalThis.msimeHarmonyBridgeReply = (id: number, reply: string) => {
  const resolve = pendingBridgeRequests.get(id);
  if (!resolve) return;
  pendingBridgeRequests.delete(id);
  resolve(reply);
};

function bridgeRequest(
  native: NativeBridge,
  kind: string,
  payload: string,
  timeoutMs = 30000,
): Promise<string> {
  const id = nextBridgeRequestId++;
  return new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => {
      if (!pendingBridgeRequests.delete(id)) return;
      reject(new Error("请求超时，请重试。"));
    }, timeoutMs);
    pendingBridgeRequests.set(id, (reply) => {
      clearTimeout(timer);
      resolve(reply);
    });
    try {
      native.startRequest(kind, id, payload);
    } catch (error) {
      clearTimeout(timer);
      pendingBridgeRequests.delete(id);
      reject(error instanceof Error ? error : new Error(String(error)));
    }
  });
}

declare global {
  interface Window {
    msimeHarmony?: NativeBridge;
  }
}

interface Reply<T> {
  ok: boolean;
  value: T;
  error: string;
}

/** 「统计」标签页的汇总，类型沿用共享统计客户端的定义。 */
type TypingSummary = Awaited<ReturnType<NonNullable<TypingStatisticsClient["summary"]>>>;

/**
 * Rejects the way the desktop host rejects: a plain record carrying a code.
 *
 * The shared settings UI decodes a failure by reading `error.code`, and only reads `message` off an
 * `Error` as a fallback. An `Error` carrying the code in its message therefore matched none of those
 * tables: every account failure arrived as the one generic sentence instead of the wording written
 * for it, a cancelled sign-in was not recognised as cancelled, a dictionary failure lost its
 * specific advice, and an AI failure put the machine code itself on screen.
 */
function unwrap<T>(raw: string): T {
  const reply = JSON.parse(raw) as Reply<T>;
  if (!reply.ok) throw { code: reply.error };
  return reply.value;
}

/** The startup failure has no settings page left to decode it, so it reads both shapes itself. */
function startupFailure(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null && "code" in error) {
    return String((error as { code: unknown }).code);
  }
  return String(error);
}

/**
 * The injected object is not guaranteed to exist the instant the document's script runs, and reading
 * it at module scope would take the whole page down with it when it does not. Wait for it instead,
 * and say so plainly if it never arrives rather than leaving a blank window to be puzzled over.
 */
function whenBridgeReady(): Promise<NativeBridge> {
  return new Promise((resolve, reject) => {
    const deadline = Date.now() + 5000;
    const poll = () => {
      const native = window.msimeHarmony;
      if (native) {
        resolve(native);
        return;
      }
      if (Date.now() > deadline) {
        reject(new Error("没有连接到水杉输入法。请从应用中打开设置。"));
        return;
      }
      setTimeout(poll, 50);
    };
    poll();
  });
}

/**
 * The host's answer to "is setup finished", once it has one.
 *
 * The reply carries `ready` because the host computes it while the window is being created and the
 * page may ask first. Waiting a moment is right where guessing is not: a default of "finished"
 * skips the welcome flow for someone who has not enabled the keyboard, and a default of
 * "unfinished" shows it to someone who has.
 *
 * It does give up. A host that never becomes ready is a host whose settings page should still open.
 */
async function whenOnboardingKnown(native: NativeBridge): Promise<boolean> {
  const deadline = Date.now() + 2000;
  for (;;) {
    try {
      const reply = JSON.parse(native.onboardingStatus()) as {
        ok: boolean;
        value: boolean;
        ready: boolean;
      };
      if (reply.ok && reply.ready) return reply.value;
    } catch {
      return false;
    }
    if (Date.now() > deadline) return false;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

/** 读取改版新增的某个同步桥接方法返回的 `{ok,value}` 回复；回复是拒绝或根本不是 JSON 时返回 undefined。 */
function bridgeValue(raw: string): unknown {
  try {
    const reply = JSON.parse(raw) as { ok?: unknown; value?: unknown };
    return reply.ok === true ? reply.value : undefined;
  } catch {
    return undefined;
  }
}

/** 宿主报告的一项设置状态：布尔值，或在系统调用没有回答时为 null。其他值都不是本页能画出的状态。 */
function setupState(value: unknown): ImeSetupState | null {
  if (typeof value === "string") {
    try {
      return setupState(JSON.parse(value));
    } catch {
      return null;
    }
  }
  if (typeof value !== "object" || value === null) return null;
  const { enabled, current } = value as { enabled?: unknown; current?: unknown };
  const isFact = (candidate: unknown): candidate is boolean | null =>
    candidate === null || typeof candidate === "boolean";
  return isFact(enabled) && isFact(current) ? { enabled, current } : null;
}

/**
 * 「设置」首页卡片、2in1 警告条和欢迎流程第一步显示的两项设置状态。
 *
 * 宿主在窗口打开时、每次回到前台时、系统当前输入法变化时以及输入法选择器回复后读取它们，并在拿到第一次结果以及任一项变化时调用 `msimeHarmonySetupChanged`——在系统界面里完成的步骤就是这样无需重新加载就反映到这里的。在第一次读取完成前就到来的订阅者还会轮询几秒，做法与启动时等待 `onboardingStatus` 相同；读取比轮询还慢时，由宿主推送的第一次结果补上。
 */
const setupListeners = new Set<(state: ImeSetupState) => void>();

globalThis.msimeHarmonySetupChanged = (document: unknown) => {
  const state = setupState(document);
  if (!state) return;
  for (const listener of setupListeners) listener(state);
};

function imeSetupClient(native: NativeBridge): ImeSetupClient | undefined {
  if (typeof native.setupStatus !== "function") return undefined;
  const read = (): ImeSetupState | null => {
    let reply: { ok?: unknown; ready?: unknown; value?: unknown };
    try {
      reply = JSON.parse(native.setupStatus()) as typeof reply;
    } catch {
      return null;
    }
    return reply.ok === true && reply.ready === true ? setupState(reply.value) : null;
  };
  let polling = false;
  const waitUntilReady = () => {
    if (polling) return;
    polling = true;
    const deadline = Date.now() + 5000;
    const poll = () => {
      const state = setupListeners.size > 0 ? read() : null;
      if (state) for (const listener of setupListeners) listener(state);
      if (state || setupListeners.size === 0 || Date.now() > deadline) {
        polling = false;
        return;
      }
      setTimeout(poll, 250);
    };
    setTimeout(poll, 250);
  };
  return {
    read,
    subscribe: (listener) => {
      setupListeners.add(listener);
      if (read() === null) waitUntilReady();
      return () => {
        setupListeners.delete(listener);
      };
    },
  };
}

/**
 * 应用主题：随季节变化的水杉四季，或固定为四季中的某一季。
 *
 * 颜色由共享的 C ABI 解析，所以本宿主画出的值与 Android 完全相同。每种外观的解析结果会一直保留，直到主题被保存，或宿主表示主题可能已变——窗口每次回到前台时它都会调用 `msimeHarmonyAppThemeChanged`，应用在后台期间跨了月份，页面就是这样得知的。被拒绝的解析结果不保留，下次渲染会重新请求。
 */
const appThemeListeners = new Set<() => void>();
const resolvedAppThemes = new Map<boolean, ResolvedAppTheme>();

function appThemeChanged() {
  resolvedAppThemes.clear();
  for (const listener of appThemeListeners) listener();
}

globalThis.msimeHarmonyAppThemeChanged = appThemeChanged;

function appThemeClient(native: NativeBridge): AppThemeClient | undefined {
  if (typeof native.appTheme !== "function") return undefined;
  // 契约用返回值而不是 rejection 表示失败——保存返回 false，解析返回 null——所以这里把拒绝当作没有结果，而不是抛进渲染里。
  const request = (action: Record<string, unknown>): unknown =>
    bridgeValue(native.appTheme(JSON.stringify(action)));
  let catalog: readonly AppThemeCatalogEntry[] | undefined;
  return {
    // 文件缺失或无法读取时，宿主的存储回复默认值，而 C ABI 的默认值是水杉四季。
    load: () => (request({ operation: "load" }) as { id: AppThemeId } | undefined)?.id ?? "siji",
    save: (id) => {
      if (request({ operation: "save", id }) === undefined) return false;
      appThemeChanged();
      return true;
    },
    resolve: (dark) => {
      const cached = resolvedAppThemes.get(dark);
      if (cached) return cached;
      const theme = request({ operation: "resolve", dark }) as ResolvedAppTheme | undefined;
      if (!theme) return null;
      resolvedAppThemes.set(dark, theme);
      return theme;
    },
    // 目录是纯计算，从不随时钟变化，所以一次成功读取就够整个页面加载期间使用。它附带的颜色表不保留：选择器通过 `resolve` 来画主题。
    catalog: () => {
      if (catalog) return catalog;
      const value = request({ operation: "catalog" }) as
        | { app_themes: AppThemeCatalogEntry[] }
        | undefined;
      if (!value) return [];
      catalog = value.app_themes.map(({ id, title, season, seasonal }) => ({
        id,
        title,
        season,
        seasonal,
      }));
      return catalog;
    },
    subscribe: (listener) => {
      appThemeListeners.add(listener);
      return () => {
        appThemeListeners.delete(listener);
      };
    },
  };
}

/** 某种外观下解析出的应用主题，主题或季节变化时重新绘制。 */
function useResolvedAppTheme(
  client: AppThemeClient | undefined,
  dark: boolean,
): ResolvedAppTheme | null {
  const [, setRevision] = useState(0);
  useEffect(() => client?.subscribe(() => setRevision((revision) => revision + 1)), [client]);
  return client?.resolve(dark) ?? null;
}

function documentDark(): boolean {
  return document.documentElement.dataset.theme === "dark";
}

/** 文档当前是否以深色绘制，以写入其 `data-theme` 的一方为准。 */
function useDocumentDark(): boolean {
  const [dark, setDark] = useState(documentDark);
  useEffect(() => {
    const update = () => setDark(documentDark());
    update();
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);
  return dark;
}

type FlowTheme = { themeMode: ThemeMode; settingsTheme: SurfaceTheme };

const SYSTEM_THEME: FlowTheme = { themeMode: "system", settingsTheme: "follow" };

/**
 * 从第一帧起按系统外观绘制文档，并在系统外观变化时跟随。
 *
 * 除此之外只有设置页会写 `data-theme`，而首次启动要等启动屏和欢迎流程结束才会进入设置页——没有 `data-theme` 的文档不管系统是什么外观，都会用深色默认值。返回停止跟随的函数，供知道用户自选外观的页面接管时调用。
 */
function followSystemTheme(): () => void {
  const apply = () => {
    document.documentElement.dataset.theme = resolveSettingsTheme(
      SYSTEM_THEME.themeMode,
      SYSTEM_THEME.settingsTheme,
    );
  };
  apply();
  if (typeof window.matchMedia !== "function") return () => undefined;
  const media = window.matchMedia("(prefers-color-scheme: light)");
  media.addEventListener("change", apply);
  return () => media.removeEventListener("change", apply);
}

/**
 * 用户为这些页面选择的外观，从已保存的文档读取，这样重放欢迎流程时会保留强制的浅色或深色，而不是跳回系统外观。文档无法读取时退回跟随系统，与首次启动相同。
 */
function savedFlowTheme(native: NativeBridge): FlowTheme {
  let snapshot: Snapshot;
  try {
    snapshot = unwrap<Snapshot>(native.loadPreferences());
  } catch {
    return SYSTEM_THEME;
  }
  const { themeMode, settingsTheme } = settingsThemePreferences(snapshot.preferences);
  return { themeMode, settingsTheme };
}

/** 欢迎闪屏是否已在本设备上播放过。宿主答不上来时就播放，在有这个标记之前每个宿主都是这么做的。 */
function splashSeen(native: NativeBridge): boolean {
  if (typeof native.splashSeen !== "function") return false;
  return bridgeValue(native.splashSeen()) === true;
}

/**
 * 键盘功能面板直接打开的页面：「词库」「反馈」和「关于」。与 `EntryAbility` 从 Want 中接受的列表相同；其他值都不是本页承诺过的链接，而改为打开「输入」（未知页面 id 就会解析到它）比停在原处更让人摸不着头脑。
 */
const LINKED_PAGES: readonly string[] = ["dictionary", "feedback", "about"];

function linkedPage(value: unknown): string | undefined {
  return typeof value === "string" && LINKED_PAGES.includes(value) ? value : undefined;
}

/** 本页加载完成前键盘请求的页面，宿主会一直保留到它被读取。 */
function pendingLinkedPage(native: NativeBridge): string | undefined {
  if (typeof native.pendingPage !== "function") return undefined;
  return linkedPage(bridgeValue(native.pendingPage()));
}

/**
 * 本窗口已打开时键盘请求打开的页面。
 *
 * 这个全局对象从本脚本运行起就存在，所以宿主不必保留在那之后发出的请求：启动屏还在时到达的请求会在这里等待，直到设置组件注册监听器后取走。
 */
let linkedPageListener: ((page: string) => void) | undefined;
let queuedLinkedPage: string | undefined;

globalThis.msimeHarmonyOpenPage = (page: string) => {
  const linked = linkedPage(page);
  if (!linked) return;
  if (linkedPageListener) linkedPageListener(linked);
  else queuedLinkedPage = linked;
};

/** Google 登录等浏览器跳回最多 300 秒（client-core 的 `GOOGLE_SIGN_IN_TIMEOUT`），之后还要用授权码换 token；默认的 30 秒会在用户还在浏览器里时就报超时。取消由页面的「取消」经 google_cancel 完成，不靠这个超时。 */
const GOOGLE_SIGN_IN_TIMEOUT_MS = 6 * 60 * 1000;

function accountClient(native: NativeBridge): AccountClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(native, "account", JSON.stringify(action)).then(unwrap<T>);
  // 头像地址原样交给页面，页面据此决定何时重新取头像；图片本身由宿主的 `avatar` 下载（页面只放行 `data:` 图片），不认可的地址在宿主那边被拒绝。
  const user = (value: {
    id: string;
    display_name: string;
    created_at: string;
    avatar_url?: string | null;
  }) => ({
    id: value.id,
    displayName: value.display_name,
    createdAt: value.created_at,
    ...(typeof value.avatar_url === "string" && value.avatar_url.startsWith("https://")
      ? { avatarUrl: value.avatar_url }
      : {}),
  });
  const profile = (value: {
    user: { id: string; display_name: string; created_at: string };
    identities: { provider: string }[];
  }) => ({
    user: user(value.user),
    providers: value.identities.map((identity) => identity.provider),
  });
  return {
    status: async () => {
      const value = await request<{
        user: { id: string; display_name: string; created_at: string } | null;
      }>({ operation: "status" });
      return { user: value.user ? user(value.user) : null };
    },
    providers: async () => {
      const value = await request<{ providers: Record<string, boolean> }>({
        operation: "providers",
      });
      return {
        email: value.providers.email === true,
        phone: value.providers.phone === true,
        apple: value.providers.apple === true,
        google: value.providers.google === true,
      };
    },
    requestCode: async (provider, target) => {
      const value = await request<{ challenge_id: string; expires_in: number }>({
        operation: "request_code",
        provider,
        target,
      });
      return { challengeId: value.challenge_id, expiresIn: value.expires_in };
    },
    login: async (challengeId, code) => {
      const value = await request<{
        user: { id: string; display_name: string; created_at: string };
      }>({ operation: "login", challenge_id: challengeId, credential: code });
      return { user: value.user ? user(value.user) : null };
    },
    // 宿主在 127.0.0.1 上监听、用系统浏览器打开 Google，等它跳回来再用授权码登录；页面只看到结果。
    googleLogin: async () => {
      const value = unwrap<{
        user: { id: string; display_name: string; created_at: string };
      }>(
        await bridgeRequest(
          native,
          "account",
          JSON.stringify({ operation: "google_sign_in" }),
          GOOGLE_SIGN_IN_TIMEOUT_MS,
        ),
      );
      return { user: value.user ? user(value.user) : null };
    },
    googleCancel: async () => {
      await request({ operation: "google_cancel" });
    },
    profile: async () =>
      profile(
        await request<{
          user: { id: string; display_name: string; created_at: string };
          identities: { provider: string }[];
        }>({ operation: "profile" }),
      ),
    rename: async (displayName) =>
      profile(
        await request<{
          user: { id: string; display_name: string; created_at: string };
          identities: { provider: string }[];
        }>({ operation: "rename", display_name: displayName }),
      ),
    // 头像的 `data:` 地址：宿主按 client-core 的头像规则从会话用户的地址下载并校验，没有头像或取不到时为 null，页面显示名字首字。
    avatar: async () =>
      (await request<{ data_url: string | null }>({ operation: "avatar" })).data_url ?? null,
    logout: async (all) => {
      await request({ operation: "logout", all });
    },
    deleteAccount: async () => {
      await request({ operation: "delete_account" });
    },
    clearExpired: async () => {
      await request({ operation: "clear_expired" });
    },
    settingsSync: settingsSyncClient(native),
  };
}

/**
 * Settings sync, which is the only account surface that writes to this device.
 *
 * The host does the mapping, not the page: which local settings are account settings is a decision
 * about this platform's preference document, and the page has no business holding a table of its
 * keys. All four calls are one bridge request each.
 *
 * `apply` names the account the values were read under. The host checks it against the live session
 * before and after the round trip, because a sign-out while the confirmation is on screen would
 * otherwise write a stranger's settings over the user's own — and nothing on that screen could put
 * them back.
 */
function settingsSyncClient(native: NativeBridge): SettingsSyncClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(native, "settings_sync", JSON.stringify(action)).then(unwrap<T>);
  return {
    schema: () => request<AccountPreferenceSchema>({ operation: "preferences_schema" }),
    load: () => request<AccountPreferences>({ operation: "preferences_load" }),
    upload: () => request<AccountPreferences>({ operation: "preferences_upload" }),
    apply: async (userId) => {
      await request({ operation: "preferences_apply", user_id: userId });
    },
  };
}

/**
 * The account-backed assistant, which signs in with the session the host already holds.
 *
 * This is not the user-configured AI service on the AI page: that one carries the user's own
 * endpoint and token and is reached through `aiAssistant`. This one has no credential to configure,
 * which is why the page offers it only once an account exists.
 *
 * The completion gets its own deadline. The host allows a model 125 seconds to answer, so a page
 * that gave up at 30 would report a timeout for a request that was about to succeed — and then the
 * reply would arrive for a number nobody is waiting on and be dropped.
 */
function chatClient(native: NativeBridge): ChatClient {
  return {
    models: async () =>
      unwrap<ChatModels>(
        await bridgeRequest(
          native,
          "chat",
          JSON.stringify({ operation: "chat", chat_operation: "models" }),
        ),
      ),
    complete: async (messages, model) =>
      unwrap<{ content: string }>(
        await bridgeRequest(
          native,
          "chat",
          JSON.stringify({ operation: "chat", chat_operation: "complete", messages, model }),
          130000,
        ),
      ).content,
  };
}

/**
 * The community skin gallery.
 *
 * Browsing is not gated on an account: the gallery is public, and a signed-out user who could not
 * look at it would have no way to decide whether an account is worth making. The host attaches the
 * session when there is one, which is what turns `owned` and `my_rating` into this user's answers.
 *
 * `download` is the only call that is more than a request. The host fetches the design, starts a
 * trial with it and imports it into the library in one step, and answers with both halves: the
 * saved skin for the library the page just grew, and the trial id the page answers 保留 or 还原
 * with afterwards.
 */
function communitySkinClient(native: NativeBridge): CommunitySkinClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(
      native,
      "community_skin",
      JSON.stringify({ operation: "community_skin", ...action }),
    ).then(unwrap<T>);
  return {
    // 我的作品 asks the host for scope "mine", which it sends with the session and fields=moderation so a removed skin carries its 已下架 badge.
    list: (offset, search, mine, category) =>
      request<CommunitySkinPage>({
        community_operation: "list",
        offset,
        search,
        ...(mine ? { scope: "mine" } : {}),
        category,
      }),
    detail: (id) => request<CommunitySkin>({ community_operation: "detail", id }),
    download: (id, name) =>
      request<CommunitySkinDownload>({ community_operation: "download", id, name }),
    rate: async (id, stars) => {
      await request({ community_operation: "rate", id, stars });
    },
    publish: async (id, name, description, design, category) => {
      await request({ community_operation: "publish", id, name, description, design, category });
    },
    setCategory: (id, category) =>
      request<CommunitySkin>({ community_operation: "set_category", id, category }),
    unpublish: async (id) => {
      await request({ community_operation: "unpublish", id });
    },
    finishTrial: async (id, keep) => {
      await request({ community_operation: "finish_trial", id, keep });
    },
    report: async (id, reason, detail) => {
      await request({ community_operation: "report", id, reason, detail });
    },
  };
}

/**
 * Community dictionaries and reply templates.
 *
 * The public list and one resource's detail read without an account, the way the skin gallery does.
 * 我的作品 and 收藏 do not: they are questions about an account, and answering them without one
 * would either be empty or be somebody else's.
 *
 * `storeReply` and `removeReply` never reach the network. They write the local file the keyboard
 * process rereads when a reply is asked for, which is the only thing the two sides of this app
 * share about the community — and it holds only what the user chose to keep.
 */
function communityResourceClient(native: NativeBridge): CommunityResourceClient {
  const request = <T,>(action: Record<string, unknown>): Promise<T> =>
    bridgeRequest(
      native,
      "community_resource",
      JSON.stringify({ operation: "community_resource", ...action }),
    ).then(unwrap<T>);
  return {
    list: (kind, scope, search, offset) =>
      request<CommunityResourcePage>({
        resource_operation: "list",
        kind,
        scope,
        search,
        offset,
      }),
    detail: (id) => request<CommunityResource>({ resource_operation: "detail", id }),
    publish: async (id, kind, name, description, content, revision) => {
      await request({
        resource_operation: "publish",
        id,
        kind,
        name,
        description,
        content,
        revision,
      });
    },
    apply: (id, resourceRevision) =>
      request<CommunityResourceApplication>({
        resource_operation: "apply",
        id,
        resource_revision: resourceRevision,
      }),
    save: async (id, saved) => {
      await request({ resource_operation: "save", id, saved });
    },
    rate: async (id, stars) => {
      await request({ resource_operation: "rate", id, stars });
    },
    unpublish: async (id) => {
      await request({ resource_operation: "unpublish", id });
    },
    storeReply: async (item) => {
      await request({ resource_operation: "store_reply", item });
    },
    removeReply: async (id) => {
      await request({ resource_operation: "remove_reply", id });
    },
    report: async (kind, id, reason, detail) => {
      await request({ resource_operation: "report", kind, id, reason, detail });
    },
  };
}

/**
 * AI skin generation, which is the one request that reports before it answers.
 *
 * Three pictures take minutes, so a run that said nothing until it finished would be
 * indistinguishable from one that had stopped. Progress arrives on its own global, the same
 * direction the preference-change notification uses, carrying the request id the page chose — a
 * stale run's counter must not drive a new one's display.
 *
 * The deadline is the sum of what the pieces are allowed: a chat completion may take 125 seconds
 * and each picture up to 200, and the three pictures run together. Giving this the ordinary 30
 * would report a timeout for a run that was working.
 */
function aiSkinClient(native: NativeBridge): AiSkinClient {
  return {
    generate: (requestId, prompt) =>
      bridgeRequest(
        native,
        "ai_skin",
        JSON.stringify({ operation: "generate", request_id: requestId, prompt }),
        360000,
      ).then(unwrap<AiSkinProposal[]>),
    cancel: async (requestId) => {
      await bridgeRequest(
        native,
        "ai_skin",
        JSON.stringify({ operation: "cancel", request_id: requestId }),
      ).then(unwrap<Record<string, never>>);
    },
    onProgress: async (listener) => {
      const previous = globalThis.msimeHarmonyAiSkinProgress;
      globalThis.msimeHarmonyAiSkinProgress = (requestId: string, completed: number) => {
        listener({ requestId, completed });
      };
      return () => {
        globalThis.msimeHarmonyAiSkinProgress = previous;
      };
    },
  };
}

/**
 * The on-device model store: the catalog models the `local` provider can run, downloaded into the app's own files directory.
 *
 * Every operation is an asynchronous bridge request, because an install blocks for the whole download on a native worker and a bridge method returning a Promise never settles here. Progress arrives on its own global as the `{id,stage,downloaded,total}` document client-core reports. The host drops the catalog's desktop-only models, and it has already turned the core's error text into the codes the page has sentences for.
 *
 * An install is given hours rather than the ordinary 30 seconds: it is a download of up to a few hundred megabytes on whatever network the phone has, and client-core reports a stalled connection itself. A deadline that fired first would report a failure for a download that then finishes.
 */
const voiceModelProgressListeners = new Set<(progress: LocalVoiceModelProgress) => void>();

globalThis.msimeHarmonyVoiceModelProgress = (document: string) => {
  let progress: LocalVoiceModelProgress;
  try {
    progress = JSON.parse(document) as LocalVoiceModelProgress;
  } catch {
    return;
  }
  for (const listener of voiceModelProgressListeners) listener(progress);
};

/**
 * Rejects the way the desktop shell's plugin commands reject: `{code, detail}`, where detail is the rule client-core reports for a refused pack or name, so `pluginErrorMessage` can say which file or entry to fix.
 */
function unwrapPlugin<T>(raw: string): T {
  const reply = JSON.parse(raw) as Reply<T> & { detail?: string };
  if (!reply.ok) throw { code: reply.error, detail: reply.detail ?? null };
  return reply.value;
}

/**
 * The 插件 page's host side. The host fills in the state root and the bundle's built-in sound packs, so the page, as on the desktop, never names a path. An import waits on the system picker for as long as the user leaves it open, so its deadline is the one the export save allows.
 */
function pluginClient(native: NativeBridge): PluginClient {
  const call = <T,>(action: Record<string, unknown>): T =>
    unwrapPlugin<T>(native.plugins(JSON.stringify(action)));
  return {
    catalog: async () => call<PluginCatalogResult>({ operation: "catalog" }),
    importPack: async (source) =>
      unwrapPlugin<PluginPackage | null>(
        await bridgeRequest(native, "plugin_import", JSON.stringify({ source }), 30 * 60 * 1000),
      ),
    remove: async (kind, id) => {
      call<null>({ operation: "remove", kind, id });
    },
    loadMentions: async () => call<MentionEntry[]>({ operation: "load_mentions" }),
    saveMentions: async (entries) => {
      call<null>({ operation: "save_mentions", entries });
    },
  };
}

function localVoiceModelClient(native: NativeBridge): LocalVoiceModelClient {
  const request = <T,>(action: Record<string, unknown>, timeoutMs?: number): Promise<T> =>
    bridgeRequest(native, "voice_local_model", JSON.stringify(action), timeoutMs).then(unwrap<T>);
  return {
    list: () => request<LocalVoiceModelList>({ operation: "list" }),
    install: (id) => request<string>({ operation: "install", id }, 6 * 60 * 60 * 1000),
    // 等用户在系统选择器里选文件，再复制和解压几百 MB，期限和下载一样放宽。
    import: (id) => request<string | null>({ operation: "import", id }, 6 * 60 * 60 * 1000),
    cancel: (id) => request<boolean>({ operation: "cancel", id }),
    remove: async (id) => {
      await request<null>({ operation: "remove", id });
    },
    onProgress: async (listener) => {
      voiceModelProgressListeners.add(listener);
      return () => {
        voiceModelProgressListeners.delete(listener);
      };
    },
  };
}

function cloudClipboardClient(native: NativeBridge, close: () => void): CloudClipboardPanelClient {
  return {
    close: async () => close(),
    copyText: async (text) => native.copyText(text),
    request: async (action) => {
      const { operation, ...payload } = action;
      const value = await bridgeRequest(
        native,
        "account",
        JSON.stringify({ operation: "clipboard", clipboard_operation: operation, ...payload }),
      );
      return unwrap<{ items?: { id: string; text: string }[]; enabled?: boolean }>(value);
    },
  };
}

type CloudDictionaryPage = "main" | "catalog" | "candidates" | "files" | "apply";

function cloudDictionaryClient(
  native: NativeBridge,
  close: () => void,
  setPage: (page: CloudDictionaryPage) => void,
): CloudDictionaryPanelClient {
  type Response = Awaited<ReturnType<CloudDictionaryPanelClient["request"]>>;
  return {
    close: async () => close(),
    back: async () => setPage("main"),
    openCatalog: async () => setPage("catalog"),
    openCandidates: async () => setPage("candidates"),
    openFiles: async () => setPage("files"),
    openApply: async () => setPage("apply"),
    snapshot: true,
    snapshotNative: true,
    exportNative: true,
    chooseSnapshotRestore: async () => {
      const snapshot = await bridgeRequest(
        native,
        "cloud_dictionary_snapshot",
        JSON.stringify({ operation: "snapshot_restore_preview", text: "" }),
        600000,
      );
      return unwrap<Response>(snapshot);
    },
    downloadToLocal: async (entry) => {
      unwrap<{ applied: boolean }>(native.cloudDictionaryDownload(JSON.stringify(entry)));
    },
    request: async (action: CloudDictionaryAction) => {
      const { operation, ...payload } = action;
      if (operation.startsWith("snapshot_")) {
        const snapshot = await bridgeRequest(
          native,
          "cloud_dictionary_snapshot",
          JSON.stringify(action),
          600000,
        );
        return unwrap<Response>(snapshot);
      }
      const value = await bridgeRequest(
        native,
        "cloud_dictionary",
        JSON.stringify({ operation: "dictionary", dictionary_operation: operation, ...payload }),
      );
      return unwrap<Response>(value);
    },
  };
}

function makeClient(
  native: NativeBridge,
  openCloudClipboard: () => void,
  openCloudDictionary: () => void,
): SettingsClient {
  const dictionaryReply = <T,>(action: Record<string, unknown>): T =>
    unwrap<T>(native.dictionary(JSON.stringify(action)));
  const dictionary: DictionaryClient = {
    list: async (offset: number, limit: number, kind?: LocalDictionaryKind, query?: string) =>
      dictionaryReply<{ entries: DictionaryEntry[]; has_more: boolean }>({
        operation: "list",
        offset,
        limit,
        ...(kind ? { kind } : {}),
        ...(query ? { query } : {}),
      }),
    edit: async (
      previous: DictionaryEntry | null,
      replacement: DictionaryEntry | null,
      request_id: string,
    ) => {
      dictionaryReply<{ applied: boolean }>({
        operation: "edit",
        previous,
        replacement,
        request_id,
      });
    },
    // The native bridge sends an import to the host in one request rather than in batches.
    maxImportFileBytes: UNBATCHED_DICTIONARY_FILE_BYTES,
    import: async (
      kind: LocalDictionaryKind,
      format: LocalDictionaryFormat,
      text: string,
      request_id: string,
    ): Promise<DictionaryImportResult> =>
      dictionaryReply<DictionaryImportResult>({
        operation: "import",
        kind,
        format,
        text,
        request_id,
      }),
    /**
     * The Apple-compatible personal dictionary file.
     *
     * Unlike the edits beside it, this one is queued rather than written: the keyboard may be open,
     * and the Engine's maintenance lock is not available while it is. The host writes the entries
     * to the queue the keyboard drains at its next session start, which is why the card says 已加入
     * 同步队列 rather than 已导入 — that is the truth on this host as it is on the Apple one.
     */
    importPersonal: async (text: string, request_id: string) =>
      dictionaryReply<{ queued: boolean; pending_count: number }>({
        operation: "import_personal",
        text,
        request_id,
      }),
    export: async (
      kind: LocalDictionaryKind,
      format: Exclude<LocalDictionaryFormat, "rime" | "hans">,
      offset: number,
      limit: number,
    ) =>
      dictionaryReply<{ text: string; has_more: boolean }>({
        operation: "export",
        kind,
        format,
        offset,
        limit,
      }),
    count: async (kind: LocalDictionaryKind) =>
      dictionaryReply<{ count: number }>({ operation: "count", kind }).count,
    retry: async (request_id: string) => {
      dictionaryReply<{ applied: boolean }>({ operation: "retry", request_id });
    },
    dismissFailure: async (request_id: string) => {
      dictionaryReply<{ applied: boolean }>({ operation: "dismiss_failure", request_id });
    },
  };
  // 命名词库。导入要解析最多 16 MiB 的文本，所以走 startRequest 在原生工作线程上执行，并给足两分钟；其余操作只改几个小文件。
  const collectionsReply = async (action: Record<string, unknown>) =>
    unwrap<DictionaryCollectionsView>(
      await bridgeRequest(native, "dictionary_collections", JSON.stringify(action), 120000),
    );
  const dictionaryCollections: DictionaryCollectionsClient = {
    load: () => collectionsReply({ operation: "load" }),
    flush: () => collectionsReply({ operation: "flush" }),
    create: (name) => collectionsReply({ operation: "create", name, kind: "pinyin" }),
    delete: (id) => collectionsReply({ operation: "delete", id }),
    setEnabled: (id, enabled) => collectionsReply({ operation: "set_enabled", id, enabled }),
    addWords: (id, entries) => collectionsReply({ operation: "add_words", id, entries }),
    importFile: (name, format, text) =>
      collectionsReply({ operation: "import", name, kind: "pinyin", format, text }),
    installCommunity: (resource) => collectionsReply({ operation: "install_community", resource }),
  };
  const userWordCount = (): number | undefined => {
    try {
      return dictionaryReply<{ count: number }>({
        operation: "count",
        kind: "pinyin",
        user_only: true,
      }).count;
    } catch {
      return undefined;
    }
  };
  const typingStatistics: TypingStatisticsClient = {
    load: async () =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "load" })),
      ),
    setEnabled: async (enabled: boolean) =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "set_enabled", enabled })),
      ),
    setRetention: async (retention: StatisticsRetention) =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "set_retention", retention })),
      ),
    reset: async () =>
      unwrap<TypingStatisticsStatus>(
        native.typingStatistics(JSON.stringify({ operation: "reset" })),
      ),
    // 「统计」标签页的派生数据。宿主补上本地日期，并直接回复存储自身的结果，而不是上面四个调用所用的状态包装；它在原生工作线程上运行，因为要在键盘的锁下读取整份文档。页面不知道用户的词数，所以在这里读取，即 Android 为「造词者」徽章发送的仅限用户词的拼音词数；词库无法读取时省略该值，存储随后按零计数。
    summary: async (userWords?: number) =>
      unwrap<TypingSummary>(
        await bridgeRequest(
          native,
          "typing_statistics",
          JSON.stringify({ operation: "summary", user_words: userWords ?? userWordCount() }),
        ),
      ),
    // 应用了一款皮肤，用于「换装达人」徽章。漏计一次应用不值得为此打断应用皮肤，所以不读取回复。
    recordSkin: async (id: string) => {
      void bridgeRequest(
        native,
        "typing_statistics",
        JSON.stringify({ operation: "record_skin", id }),
      ).catch(() => undefined);
    },
  };
  // Every call answers with the whole status; the native operation runs on a worker and the local
  // day is resolved on the ArkTS side, which is the process that knows the device's timezone.
  const vocabularyReview: VocabularyReviewClient = {
    load: async () =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(native, "vocabulary_review", JSON.stringify({ operation: "load" })),
      ),
    answer: async (word: string, known: boolean) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(
          native,
          "vocabulary_review",
          JSON.stringify({ operation: "answer", word, known }),
        ),
      ),
    setSettings: async (settings) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(
          native,
          "vocabulary_review",
          JSON.stringify({
            operation: "set_settings",
            wordbook: settings.wordbook,
            new_per_day: settings.newPerDay,
            session_limit: settings.sessionLimit,
          }),
        ),
      ),
    importWordbook: async (name: string, text: string) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(
          native,
          "vocabulary_review",
          JSON.stringify({ operation: "import", name, text }),
        ),
      ),
    removeWordbook: async (wordbook: string) =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(
          native,
          "vocabulary_review",
          JSON.stringify({ operation: "remove", wordbook }),
        ),
      ),
    reset: async () =>
      unwrap<VocabularyReviewStatus>(
        await bridgeRequest(native, "vocabulary_review", JSON.stringify({ operation: "reset" })),
      ),
  };
  const aiAssistant: AiAssistantClient = {
    fetchModels: (configuration) =>
      bridgeRequest(native, "ai_models", JSON.stringify(configuration)).then(unwrap<string[]>),
    test: (configuration) =>
      bridgeRequest(native, "ai_test", JSON.stringify(configuration)).then(unwrap<string>),
  };
  // 只读一次：能力记录在应用运行期间不会变化，下面的反馈报告据此写明版本。
  const host = unwrap<HostCapabilities>(native.hostCapabilities());
  const setup = imeSetupClient(native);
  // 状态栏和导航栏属于窗口，只有宿主够得着。
  const chrome: HostChromeClient | undefined =
    typeof native.setSystemBars === "function"
      ? { setSystemBars: (bars) => native.setSystemBars(JSON.stringify(bars)) }
      : undefined;
  // 「反馈」经账号桥接提交：有人登录时桥接附上已登录的会话，无人登录时附上设备的匿名账号会话（宿主在原生层持有，与 Android 一致）。只有两者都没有时（例如匿名注册还没成功）提交才会以 `account_unauthorized` 被拒绝，页面会如实提示。
  const feedback: FeedbackClient = {
    submit: async (report) => {
      unwrap<Record<string, never>>(
        await bridgeRequest(
          native,
          "account",
          JSON.stringify({
            operation: "feedback",
            type: report.type,
            text: report.text,
            diagnostics: report.diagnostics,
            app_version: native.appVersion(),
            edition: host.edition?.id ?? "full",
          }),
        ),
      );
    },
  };
  const testApiCredential = async (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ): Promise<ApiCredentialTestResult> =>
    unwrap<ApiCredentialTestResult>(
      await bridgeRequest(native, "api_credential", JSON.stringify({ service, config })),
    );
  return {
    // Wrapped like every other reply from the shared ABI. Reading it as the record itself leaves every
    // capability undefined, which the page reads as "this host cannot", and the whole surface silently
    // shrinks to the few controls that have no capability behind them.
    host,
    load: async () => unwrap<Snapshot>(native.loadPreferences()),
    // The keyboard writes preferences from its toolbar and the two are separate processes, so the
    // host calls this when the settings window comes back to the front and the document has moved.
    // The reply is the one loadPreferences would have returned, so both paths parse identically.
    onPreferencesChanged: async (listener) => {
      globalThis.msimeHarmonyPreferencesChanged = (reply: string) => {
        try {
          listener(unwrap<Snapshot>(reply));
        } catch {
          // A document this cannot read is not worth interrupting the page for; the next save still
          // has the store's revision check behind it.
        }
      };
      return () => {
        globalThis.msimeHarmonyPreferencesChanged = undefined;
      };
    },
    save: async (revision: number, preferences: Preferences) => {
      // The revision sent is the one the page read; the document carries the next. The store compares
      // the former against what is on disk and refuses the save if the keyboard moved in between.
      //
      // `format_version` is not optional: the C ABI parses this document as `PreferencesSnapshot`,
      // whose field is required and which denies unknown fields, so a document without it fails to
      // parse and comes back as `invalid preferences snapshot` — which this host maps to `format`,
      // the "配置文件无法读取或版本较新" banner. Leaving it out meant no save on HarmonyOS had ever
      // reached the disk: the page reported nothing, the banner sat above the fold, and
      // preferences.json was never created. Android and Apple both send it.
      const document = JSON.stringify({ format_version: 1, revision: revision + 1, preferences });
      return unwrap<Snapshot>(native.savePreferences(revision, document));
    },
    readAppVersion: async () => native.appVersion(),
    // GitHub's release list is read and compared in Rust on a native worker (msime_client_update_check); ArkTS fills in the platform.
    checkUpdate: async (request: UpdateCheckRequest) =>
      unwrap<UpdateCheckResult>(
        await bridgeRequest(
          native,
          "update_check",
          JSON.stringify({
            current_version: request.currentVersion,
            ...(request.edition === undefined ? {} : { edition: request.edition }),
            ...(request.arch === undefined ? {} : { arch: request.arch }),
          }),
        ),
      ),
    scanSkinCatalog: async () => unwrap<SkinCatalog>(native.scanSkinCatalog()),
    readSkinImage: async (id: string, relative: string) =>
      unwrap<SkinImage>(native.readSkinImage(id, relative)),
    readSkinFont: async (id: string, relative: string) =>
      unwrap<SkinFont>(native.readSkinFont(id, relative)),
    readSkinToolbarCss: async (id: string, relative?: string) =>
      relative ? null : unwrap<string | null>(native.readSkinToolbarCss(id)),
    openExternalUrl: async (url: string) => native.openExternalUrl(url),
    copyText: async (text: string) => native.copyText(text),
    openSystemKeyboardSettings: async () => native.openSystemKeyboardSettings(),
    // The source opens its skin folder so a skin can be dropped in. That folder is inside the
    // sandbox here, so the direction is reversed: the user points at a skin and it is copied in.
    // The page renders this behind the same control and says 导入皮肤 instead, because the host
    // capability tells it which of the two this is.
    openSkinDirectory: async () => {
      native.importSkinFolder();
    },
    // ArkWeb drops the page's download link, so the host saves the export through the system save picker instead. The deadline covers a user who leaves the picker open: timing out under them would report a failure for a file that is then written anyway.
    saveExport: async (name: string, contents: string) => {
      const reply = await bridgeRequest(
        native,
        "save_export",
        JSON.stringify({ name, contents }),
        30 * 60 * 1000,
      );
      try {
        return unwrap<string | null>(reply);
      } catch {
        throw new Error("无法保存导出文件，词库未导出。");
      }
    },
    listVoiceCaptureDevices: async () =>
      unwrap<VoiceCaptureDevice[]>(native.listVoiceCaptureDevices()),
    listFontFamilies: async () => unwrap<string[]>(native.listFontFamilies()),
    mobileKeyboardFeedback: {
      load: async () =>
        unwrap<MobileKeyboardFeedback>(
          native.keyboardFeedback(JSON.stringify({ operation: "load" })),
        ),
      save: async (settings) =>
        unwrap<MobileKeyboardFeedback>(
          native.keyboardFeedback(JSON.stringify({ operation: "save", settings })),
        ),
      preview: async (strength) => {
        unwrap<MobileKeyboardFeedback>(
          native.keyboardFeedback(JSON.stringify({ operation: "preview", strength })),
        );
      },
    },
    dictionary,
    dictionaryCollections,
    typingStatistics,
    vocabularyReview,
    aiAssistant,
    testApiCredential,
    // Four surfaces the keyboard already honours. Each writes shared preferences and nothing else,
    // so opting in is all that was ever needed; without it the page saved nothing and the keyboard
    // went on reading defaults the user had no way to change.
    fuzzyPinyin: true,
    touchKeyboardSchemes: true,
    customTouchKeyboardSkins: true,
    // The manifest is packaged and never changes while the application runs, so this is read on
    // demand rather than kept: the dictionary page is not a screen anyone leaves open.
    dictionaryManifest: async () => unwrap<DictionaryManifest>(native.dictionaryManifest()),
    // A named design can carry a bounded photo. Both reads and writes use the numbered request
    // channel while the native worker parses and writes the library under its file lock.
    customSkinLibrary: {
      load: async () =>
        unwrap<SavedTouchKeyboardSkin[]>(await bridgeRequest(native, "custom_skin_library", "")),
      mutate: async (action) =>
        unwrap<SavedTouchKeyboardSkin[]>(
          await bridgeRequest(native, "custom_skin_library", JSON.stringify(action)),
        ),
    },
    candidateEnglishGloss: true,
    account: accountClient(native),
    chat: chatClient(native),
    // The Apple home surface, adapted rather than copied. Two of its five actions exist here and
    // three do not, and the page draws only what the host says it has.
    //
    // The two setup actions are this host's: 设置 opens the system input-method list, and the
    // picker is where the second setup step happens.
    //
    // `openKeyboard` is deliberately absent. Android opens a separate panel window for it; this
    // host's keyboard is an InputMethodExtensionAbility that appears when an editor asks for it,
    // and there is no window for the settings app to open. Without the action the card falls back
    // to the shared screen-keyboard page, which is the honest version of "show me the keyboard"
    // here. The emoji and clipboard actions are absent for the same reason: on this host those are
    // surfaces on the keyboard's own key faces, not windows.
    home: {
      openSystemKeyboardSettings: async () => native.openSystemKeyboardSettings(),
      // The reply is unwrapped rather than ignored so a host that could not open the picker says
      // so, which the card reports; the welcome flow's own copy of this call is the exception,
      // because that screen has its own failure to show and nothing to add to it.
      showInputMethodPicker: async () => {
        unwrap<boolean>(native.showInputMethodPicker());
      },
      // 「设置」卡片各项检查背后的两项设置状态，在 2in1 上也是「输入」和「关于」页警告条的依据。2in1 没有首页要画，但仍需要这些操作：警告条调用的是同样两个。
      setup,
    },
    appTheme: appThemeClient(native),
    chrome,
    feedback,
    communitySkins: communitySkinClient(native),
    communityResources: communityResourceClient(native),
    aiSkins: aiSkinClient(native),
    localVoiceModels: localVoiceModelClient(native),
    // The page is offered only on a 2in1, the form factor that plays packs and routes the / and @ modes; a phone hides it whatever the host supplies.
    plugins: pluginClient(native),
    openCloudClipboard: async () => openCloudClipboard(),
    cloudClipboardRequest: cloudClipboardClient(native, () => undefined).request,
    openCloudDictionary: async () => openCloudDictionary(),
  };
}

type OnboardingFinish = (
  scheme: OnboardingInputScheme,
  choices: OnboardingChoices,
) => Promise<void>;

/**
 * 欢迎流程，连同本宿主能告诉它的信息。
 *
 * 流程显示期间由它掌管文档主题：平时写 `data-theme` 的设置页没有挂载，而流程应当以用户在其他地方看到的外观打开，而不是样式表的深色默认值。在手机上它还会取应用主题的季节，让第一屏就用上设置页将要使用的颜色。
 */
function HarmonyWelcomeFlow({
  native,
  client,
  splash,
  theme,
  onComplete,
  onSkip,
}: {
  native: NativeBridge;
  client: SettingsClient;
  splash: boolean;
  theme: FlowTheme;
  onComplete: OnboardingFinish;
  onSkip: OnboardingFinish;
}): ReactNode {
  useSettingsTheme(theme.themeMode, theme.settingsTheme);
  const dark = useDocumentDark();
  // 2-in-1 报告 `mobile_settings` 为 false：流程在那里画成桌面式面板，不显示手机的闪屏，并保留自己的中性背景。
  const mobileSettings = client.host?.mobile_settings;
  const appTheme = useResolvedAppTheme(
    mobileSettings !== false ? client.appTheme : undefined,
    dark,
  );
  // 最后一步只向未登录的人提供「登录」。判断需要一次网络往返，所以流程不等结果就打开，结果到达后再更新；失败按未登录处理，最多只是让这一步多提供一个其实不需要的登录入口。
  const [signedIn, setSignedIn] = useState<boolean>();
  useEffect(() => {
    const account = client.account;
    if (!account) return;
    let current = true;
    account.status().then(
      (status) => {
        if (current) setSignedIn(status.user !== null);
      },
      () => {
        if (current) setSignedIn(false);
      },
    );
    return () => {
      current = false;
    };
  }, [client]);
  // 「五笔」卡片显示正在使用的码表，这是已保存的偏好，不是流程所选。文档无法读取时卡片保留默认文案；这类失败在保存流程的选择时报告。
  const [wubiProfile, setWubiProfile] = useState<Preferences["wubi_profile"]>();
  useEffect(() => {
    let current = true;
    client.load().then(
      (snapshot) => {
        if (current) setWubiProfile(snapshot.preferences.wubi_profile);
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [client]);
  // 在闪屏开始而不是结束时标记，与 Android 相同，这样闪屏中途被关掉的那次启动不会再播一遍。
  useEffect(() => {
    if (splash && typeof native.markSplashSeen === "function") native.markSplashSeen();
  }, [native, splash]);
  return (
    <WelcomeFlowPage
      actions={{
        platform: "harmony",
        mobileSettings,
        // 资源由键盘启动时就绪，这里没有单独的步骤要执行；流程需要的是这个 promise，而不是实际工作。
        prepareResources: async () => {},
        openSystemKeyboardSettings: async () => native.openSystemKeyboardSettings(),
        showInputMethodPicker: async () => {
          native.showInputMethodPicker();
        },
      }}
      // 方案步骤只提供本版本和本宿主具备的键盘。
      edition={client.host?.edition}
      inputSchemes={client.host?.input_schemes}
      wubiProfile={wubiProfile}
      setup={client.home?.setup}
      signedIn={signedIn}
      appTheme={appTheme}
      chrome={client.chrome}
      onComplete={onComplete}
      onSkip={onSkip}
      splash={splash}
    />
  );
}

function HarmonySettings({
  native,
  onboarding,
  splashUnseen,
  linkedPage,
}: {
  native: NativeBridge;
  onboarding: boolean;
  /** 闪屏尚未在本设备上播放过，取自宿主在启动时的报告。 */
  splashUnseen: boolean;
  /** 键盘要求本窗口打开的页面，启动时从宿主读取。 */
  linkedPage: string | undefined;
}): ReactNode {
  // Setup is the one thing that has to happen before anything in the settings page can matter, so
  // the flow replaces the page rather than sitting somewhere inside it. Skipping is allowed: a
  // keyboard the user has decided to set up later is not a reason to withhold its settings.
  const [bootstrapRequired, setBootstrapRequired] = useState(onboarding);
  // The splash belongs to a first launch; a flow replayed from settings opens on its first step.
  const [replayed, setReplayed] = useState(false);
  // 流程绘制所用的外观：首次启动用系统外观，重放时用用户自己的选择。
  const [flowTheme, setFlowTheme] = useState<FlowTheme>(SYSTEM_THEME);
  // 设置打开时所在的页面：键盘链接到的页面，或流程最后「登录」之后的「我的」，登录入口就在那里。
  const [initialPage, setInitialPage] = useState<string | undefined>(linkedPage);
  // 设置已打开时键盘请求的页面。新的 nonce 会导航过去并保留草稿；若改为按初始页面重新挂载，草稿就会丢失。
  const [route, setRoute] = useState<{ page: string; nonce: number }>();
  const [cloudClipboardOpen, setCloudClipboardOpen] = useState(false);
  const [cloudDictionaryOpen, setCloudDictionaryOpen] = useState(false);
  const [cloudDictionaryPage, setCloudDictionaryPage] = useState<CloudDictionaryPage>("main");
  // Each client is built once per bridge. SettingsPage and the cloud panels key their load and subscription effects on the client, so a fresh one on every render (opening a cloud panel re-renders this component) reloaded the page and threw away the unsaved draft. The callbacks only close over state setters, which React keeps stable.
  const client = useMemo(
    () =>
      makeClient(
        native,
        () => setCloudClipboardOpen(true),
        () => {
          setCloudDictionaryPage("main");
          setCloudDictionaryOpen(true);
        },
      ),
    [native],
  );
  const cloudClipboard = useMemo(
    () => cloudClipboardClient(native, () => setCloudClipboardOpen(false)),
    [native],
  );
  const dictionaryClient = useMemo(
    () =>
      cloudDictionaryClient(native, () => setCloudDictionaryOpen(false), setCloudDictionaryPage),
    [native],
  );
  const filesClient = useMemo<CloudDictionaryPanelClient>(
    () => ({
      ...dictionaryClient,
      snapshot: true,
      snapshotNative: true,
    }),
    [dictionaryClient],
  );
  // 欢迎流程显示期间，链接的页面等设置打开时再作为初始页；设置已打开时则直接导航过去。在这个监听器存在之前到达的请求现在取走。
  useEffect(() => {
    const open = (page: string) => {
      if (bootstrapRequired) setInitialPage(page);
      else setRoute({ page, nonce: Date.now() });
    };
    linkedPageListener = open;
    const queued = queuedLinkedPage;
    queuedLinkedPage = undefined;
    if (queued) open(queued);
    return () => {
      linkedPageListener = undefined;
    };
  }, [bootstrapRequired]);
  if (bootstrapRequired) {
    // 流程中选的方案正是那一步的意义所在；丢掉它会让用户的键盘仍是他们刚刚放弃的布局。写入方式与移动端宿主相同，这样在它们之间同步的配置含义一致。「跳过」也保留它：提前离开引导不等于放弃已在其中做出的选择。
    const applyChoices = async (scheme: OnboardingInputScheme, choices: OnboardingChoices) => {
      const snapshot = await client.load();
      await client.save(
        snapshot.revision,
        completeOnboardingPreferences(snapshot, scheme, choices),
      );
    };
    // 流程显示期间宿主会暂缓显示通知卡片；这里让它恢复。
    const leaveFlow = () => {
      if (typeof native.onboardingFinished === "function") native.onboardingFinished();
      setBootstrapRequired(false);
    };
    return (
      <HarmonyWelcomeFlow
        native={native}
        client={client}
        splash={client.host?.mobile_settings !== false && !replayed && splashUnseen}
        theme={flowTheme}
        onComplete={async (scheme, choices) => {
          await applyChoices(scheme, choices);
          if (choices.openAccount) setInitialPage("account");
          leaveFlow();
        }}
        onSkip={async (scheme, choices) => {
          // 「跳过」是离开流程的出口，而流程后面的设置页才是报告和恢复无法读取的文档的地方。因此，已做选择写入失败（文档无法读取，或保存时与键盘发生竞争）不会把用户卡在这里：无论如何都离开流程，设置页加载文档时会报告文档无法读取。
          await applyChoices(scheme, choices).catch(() => undefined);
          leaveFlow();
        }}
      />
    );
  }
  return (
    <>
      <SettingsPage
        client={client}
        initialPage={initialPage}
        route={route}
        onReplayOnboarding={() => {
          setFlowTheme(savedFlowTheme(native));
          // 重放与首次运行在同一处结束，而不是停在之前链接的页面上。
          setInitialPage(undefined);
          setReplayed(true);
          // 重放的流程同样不应被宿主的通知卡片压在下面；离开流程时 leaveFlow 会让它恢复。
          if (typeof native.onboardingStarted === "function") native.onboardingStarted();
          setBootstrapRequired(true);
        }}
      />
      {cloudClipboardOpen && <CloudClipboardPanel client={cloudClipboard} />}
      {cloudDictionaryOpen && cloudDictionaryPage === "main" && (
        <CloudDictionaryPanel client={dictionaryClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "catalog" && (
        <CloudDictionaryCatalogPanel client={dictionaryClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "candidates" && (
        <CloudCandidatesPanel client={dictionaryClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "files" && (
        <CloudDictionaryFilesPanel client={filesClient} />
      )}
      {cloudDictionaryOpen && cloudDictionaryPage === "apply" && (
        <CloudDictionaryApplyPanel client={dictionaryClient} />
      )}
    </>
  );
}

const root = document.getElementById("root");
if (root) {
  const app = createRoot(root);
  // 启动页是第一帧，所以必须在它渲染前设好主题。它跟随系统，直到知道用户自选外观的欢迎流程或设置页接管文档。
  const stopFollowingSystemTheme = followSystemTheme();
  // Waiting for the bridge took up to five seconds against a blank white window. Every other host
  // shows the shared startup page while it opens; there was never a reason for this one not to.
  app.render(
    <StrictMode>
      <SettingsStartupPage />
    </StrictMode>,
  );
  whenBridgeReady()
    .then(async (native) => {
      // A refused query answers "no onboarding": someone who has been using the keyboard for weeks
      // should not be sent back to a welcome screen because one system call did not answer.
      const onboarding = await whenOnboardingKnown(native);
      // 在页面表明欢迎流程不会挡路之前，宿主会暂缓显示通知卡片。页面直接打开设置时（包括等首次读取等太久而放弃之后），不会再有别的地方告诉它。
      if (!onboarding && typeof native.onboardingFinished === "function")
        native.onboardingFinished();
      // 两者都在这里、在 React 之外读取，因为都是一次性的宿主读取：链接的页面只交出一次，而闪屏标记马上就要写入。
      const linkedPage = pendingLinkedPage(native);
      const splashUnseen = !splashSeen(native);
      stopFollowingSystemTheme();
      app.render(
        <StrictMode>
          <HarmonySettings
            native={native}
            onboarding={onboarding}
            splashUnseen={splashUnseen}
            linkedPage={linkedPage}
          />
        </StrictMode>,
      );
    })
    .catch((error: unknown) => {
      // A blank window explains nothing. This is the one failure the page has to render itself,
      // because it is the failure that means none of the rest of it can be rendered at all. Both
      // rejection shapes reach here: the bridge itself refuses with an Error carrying a sentence,
      // while a refused host call arrives as a record carrying a code.
      root.textContent = startupFailure(error);
      root.setAttribute("style", "padding:24px;font:16px system-ui;color:#c0392b");
    });
}
