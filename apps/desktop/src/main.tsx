import { createTauriProgressClient } from "./core/tauri-progress-client";
import { StrictMode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { createMobileSettingsHistoryState } from "../../../packages/ui/src/settings/mobile-history-state";
import { trackAsyncSubscription } from "../../../packages/ui/src/core/async-subscription";
import { getVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  CloudCandidatesPanel,
  CloudClipboardPanel,
  CloudDictionaryApplyPanel,
  CloudDictionaryCatalogPanel,
  CloudDictionaryFilesPanel,
  CloudDictionaryPanel,
  VoicePanel,
  completeOnboardingPreferences,
  useMobilePopState,
  subscribeToWindowEvent,
  SettingsPage,
  SettingsStartupPage,
  WelcomeFlowPage,
  LinuxSetupPage,
  MacosInstallPage,
  savedModelMirror,
  type AccountClient,
  type AccountProfile,
  type AccountQqLink,
  type QqLinksClient,
  type ApiCredentialTestResult,
  type ApiCredentialTestService,
  type CloudDictionaryAction,
  type CloudDictionaryPanelClient,
  type CustomHelpcodeSchema,
  type HostCapabilities,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
  type VoiceCredentialSaveResult,
  type TypingStatisticsClient,
  type TypingStatisticsUploadClient,
  VocabularyReviewPanel,
  type VocabularyReviewClient,
  type PanelClient,
  type Preferences,
  type PreferencesRecovery,
  type AppNotice,
  type UpdateCheckResult,
  type SettingsClient,
  type Snapshot,
  type DictionaryClient,
  type DictionaryEntry,
  type LocalDictionaryKind,
  type LocalDictionaryFormat,
  type OnboardingActions,
  type OnboardingChoices,
  type OnboardingInputScheme,
  type LinuxSetupClient,
  type MacosInstallClient,
  type LinuxSetupLine,
  type LinuxSetupStage,
  type LinuxSetupStatus,
  type McpClientId,
  type McpFlag,
  type McpInstallOutcome,
  type McpServerStatus,
  type LocalVoiceModelClient,
  type LocalVoiceModelList,
  type LocalVoiceModelProgress,
  type MentionEntry,
  type ResourcePackClient,
  type ResourcePackStatus,
  type PluginCatalogResult,
  type PluginClient,
  type PluginPackage,
  type ScriptGrantsResult,
  type ScelText,
  type WubiCodeLookup,
  UNBATCHED_DICTIONARY_FILE_BYTES,
  StatusMessage,
} from "@msime/ui";
import "@msime/ui/styles.css";
import { subscribeWindowState } from "./input/window-state";
import { discoverFontReader } from "./candidate/system-font-client";
import { DesktopKeyboard } from "./input/desktop-keyboard";
import { createDesktopPanelClients } from "./input/desktop-panel-clients";
import { createDesktopClipboardClient } from "./input/desktop-clipboard-client";
import { useHostPlatform, usePanelHost } from "./input/panel-host";
import { DesktopPanelTheme } from "./input/desktop-panel-theme";
import { DesktopEmojiPanel } from "./input/desktop-emoji-panel";
import { DesktopHandwriting } from "./input/desktop-handwriting";
import { DesktopCloudDictionary } from "./dictionary/desktop-cloud-dictionary";
import { createDesktopCloudDictionaryClient } from "./dictionary/desktop-cloud-dictionary-client";
import { testDesktopApiCredential } from "./account/credential-test-client";
import { createAccountCommandClient } from "./account/account-command-client";
import { isMobileHost } from "./input/mobile-host-capabilities";
import { createMobileHostServices } from "./core/mobile-host-services";
import {
  createDesktopCandidateSkinCommunity,
  createDesktopCommunityResources,
  createDesktopPluginCommunity,
  createDesktopSettingsSync,
} from "./core/desktop-host-services";

const dictionary: DictionaryClient = {
  browse: (offset, limit, kind, query) =>
    invoke("dictionary_request", {
      action: { operation: "browse", offset, limit, kind, query },
    }),
  updates: (command, enabled) =>
    invoke("dictionary_request", { action: { operation: "updates", command, enabled } }),
  list: (offset, limit, kind, query) =>
    invoke("dictionary_request", {
      action: { operation: "list", offset, limit, kind, query },
    }),
  edit: (
    previous: DictionaryEntry | null,
    replacement: DictionaryEntry | null,
    request_id: string,
  ) =>
    invoke("dictionary_request", {
      action: { operation: "edit", previous, replacement, request_id },
    }).then(() => undefined),
  import: (
    kind: LocalDictionaryKind,
    format: LocalDictionaryFormat,
    text: string,
    request_id: string,
  ) =>
    invoke("dictionary_request", {
      action: { operation: "import", kind, format, text, request_id },
    }),
  export: (
    kind: LocalDictionaryKind,
    format: Exclude<LocalDictionaryFormat, "rime" | "hans">,
    offset: number,
    limit: number,
  ) =>
    invoke("dictionary_request", { action: { operation: "export", kind, format, offset, limit } }),
  retry: (request_id) =>
    invoke("dictionary_request", { action: { operation: "retry", request_id } }).then(
      () => undefined,
    ),
  dismissFailure: (request_id) =>
    invoke("dictionary_request", { action: { operation: "dismiss_failure", request_id } }).then(
      () => undefined,
    ),
  // 只读码表。Android 的 `dictionary_request` 把它交给个人词库入口，那里直接回答、不进队列；手机端经 `mobileDictionary` 一并带上。
  wubiCodes: (text: string) =>
    invoke<WubiCodeLookup>("dictionary_request", { action: { operation: "wubi_codes", text } }),
  // 细胞词库转成标准文本是纯函数，各平台的外壳都有这条命令；页面再把文本当 `standard` 交给 `import`。
  scelText: (bytesBase64: string) => invoke<ScelText>("dictionary_scel_text", { bytesBase64 }),
};
const mobileDictionary: DictionaryClient = {
  ...dictionary,
  // The mobile bridge sends an import to the host in one request rather than in batches.
  maxImportFileBytes: UNBATCHED_DICTIONARY_FILE_BYTES,
  importPersonal: (text: string, request_id: string) =>
    invoke("dictionary_request", { action: { operation: "import_personal", text, request_id } }),
};

/** 在配置脚本运行期间转发它的输出和阶段事件，结束后取消订阅。 */
async function withLinuxSetupEvents(
  onLine: (line: LinuxSetupLine) => void,
  onStage: ((stage: LinuxSetupStage) => void) | undefined,
  command: () => Promise<LinuxSetupStatus>,
): Promise<LinuxSetupStatus> {
  const unlistenOutput = await listen<LinuxSetupLine>("linux-setup-output", (event) =>
    onLine(event.payload),
  );
  const unlistenStage = await listen<LinuxSetupStage>("linux-setup-stage", (event) =>
    onStage?.(event.payload),
  ).catch((error: unknown) => {
    unlistenOutput();
    throw error;
  });
  try {
    return await command();
  } finally {
    unlistenOutput();
    unlistenStage();
  }
}
const linuxSetupClient: LinuxSetupClient = {
  run: ({ download, cloudCandidates }, onLine, onStage) =>
    withLinuxSetupEvents(onLine, onStage, () =>
      invoke<LinuxSetupStatus>("run_linux_setup", { download, cloudCandidates }),
    ),
  update: (onLine, onStage) =>
    withLinuxSetupEvents(onLine, onStage, () =>
      invoke<LinuxSetupStatus>("run_linux_dictionary_update"),
    ),
  close: () => getCurrentWindow().close(),
  openFcitx5Config: () => invoke<void>("open_fcitx5_configtool"),
  copyText: (text) => invoke<void>("copy_text", { text }),
};
// The host resolves the plugins directory and the built-in sound packs, and shows its own picker for an import; the page never names a path.
const plugins: PluginClient = {
  catalog: () => invoke<PluginCatalogResult>("plugin_catalog"),
  importPack: (source) => invoke<PluginPackage | null>("import_plugin_pack", { source }),
  remove: (kind, id) => invoke<void>("remove_plugin_pack", { kind, id }),
  loadMentions: () => invoke<MentionEntry[]>("load_plugin_mentions"),
  saveMentions: (entries) => invoke<void>("save_plugin_mentions", { entries }),
  // 脚本插件的授权：设置应用经 client-services 的 `script_grants` 写入。页面只在宿主声明了 `script_plugins` 时调用。
  scriptGrants: () => invoke<ScriptGrantsResult>("script_grants"),
  setScriptGrant: (id, hash, enabled, granted) =>
    invoke<ScriptGrantsResult>("set_script_grant", { id, hash, enabled, granted }),
  reorderScripts: (ids) => invoke<ScriptGrantsResult>("reorder_scripts", { ids }),
};
const typingStatistics: TypingStatisticsClient = {
  load: () => invoke("load_typing_statistics"),
  setEnabled: (enabled: boolean) => invoke("set_typing_statistics_enabled", { enabled }),
  setRetention: (retention: string) => invoke("set_typing_statistics_retention", { retention }),
  reset: () => invoke("reset_typing_statistics"),
};
// 「上传到云端」：节流（十分钟一次）、构建和状态文件都在 Rust 侧，页面只开关和触发。
const typingStatisticsUpload: TypingStatisticsUploadClient = {
  status: () => invoke("typing_statistics_upload_status"),
  setEnabled: (enabled: boolean) => invoke("set_typing_statistics_upload", { enabled }),
  sync: () => invoke("upload_typing_statistics"),
};
const cloudDictionaryClient: CloudDictionaryPanelClient = {
  close: () => invoke("close_panel", { label: "cloud-dictionary-panel" }),
  request: (action: CloudDictionaryAction) => invoke("cloud_dictionary_request", { action }),
};
// 宿主把云词库导出写进「下载」，见 `createDesktopCloudDictionaryClient`；只有 Linux 的面板用它。
const saveCloudDictionaryExport = (name: string, contents: string) =>
  invoke<string>("save_export", { name, contents });
// 「绑定 QQ 群助手」：Rust 侧回的是服务端的 snake_case，这里换成页面的写法。
type QqLinkDto = { id: string; created_at: string };
const qqLink = (link: QqLinkDto): AccountQqLink => ({ id: link.id, createdAt: link.created_at });
const qqLinks: QqLinksClient = {
  list: async () => (await invoke<{ links: QqLinkDto[] }>("account_qq_links")).links.map(qqLink),
  redeem: async (code) => qqLink(await invoke<QqLinkDto>("account_qq_link_redeem", { code })),
  remove: (id) => invoke<void>("account_qq_link_delete", { id }),
};
/** 账号会话接在 Tauri 设置应用里的三个桌面宿主。 */
function isDesktopAccountHost(platform: string): boolean {
  return platform === "windows" || platform === "macos" || platform === "linux";
}
// Every method answers with the whole status, so the page keeps one request in flight rather than
// following each change with a read of its own. The local day is resolved on the Rust side, which
// is the process that knows the machine's timezone.
const vocabularyReview: VocabularyReviewClient = {
  load: () => invoke("load_vocabulary_review"),
  answer: (word, known) => invoke("answer_vocabulary_card", { word, known }),
  setSettings: (settings) =>
    invoke("set_vocabulary_settings", {
      wordbook: settings.wordbook,
      newPerDay: settings.newPerDay,
      sessionLimit: settings.sessionLimit,
    }),
  importWordbook: (name, text) => invoke("import_vocabulary_wordbook", { name, text }),
  removeWordbook: (wordbook) => invoke("remove_vocabulary_wordbook", { wordbook }),
  reset: () => invoke("reset_vocabulary_review"),
};
const inputSourceStartup: NonNullable<SettingsClient["inputSourceStartup"]> = {
  status: () => invoke("input_source_startup_status"),
  openSettings: () => invoke("open_input_source_settings"),
};
const macosInputModes: NonNullable<SettingsClient["macosInputModes"]> = {
  enabled: () => invoke("enabled_input_modes"),
  openSettings: () => invoke("open_input_source_settings"),
};
// 桌面按需下载的资源包（macOS 的日文词库和「粤语、注音与笔画词库」，三个桌面平台的手写模型和桌面神经联想模型）；这些命令只在桌面宿主上注册，所以只在宿主报告桌面平台时提供给页面。本机提供哪些由宿主的列表决定。
const resourcePacks: ResourcePackClient = {
  ...createTauriProgressClient<ResourcePackStatus[], LocalVoiceModelProgress>(invoke, listen, {
    list: "resource_packs",
    install: "resource_pack_install",
    cancel: "resource_pack_cancel",
    progress: "resource-pack-progress",
  }),
  remove: (id) => invoke<boolean>("resource_pack_remove", { id }),
};
const macosInstallClient: MacosInstallClient = {
  install: (scope) => invoke("run_first_input_source_install", { scope }),
};
const localVoiceModels: LocalVoiceModelClient = {
  ...createTauriProgressClient<LocalVoiceModelList, LocalVoiceModelProgress>(invoke, listen, {
    list: "voice_local_models",
    install: "voice_local_model_install",
    cancel: "voice_local_model_cancel",
    progress: "voice-local-model-progress",
  }),
  remove: (id) => invoke<void>("voice_local_model_remove", { id }),
};

const clipboardClient = createDesktopClipboardClient();
const client: SettingsClient = {
  readAppVersion: getVersion,
  // 发布日期由 src-tauri/build.rs 在编译时写进设置应用，拿不到时是空字符串。
  readAppReleaseDate: () => invoke<string>("app_release_date"),
  resolveFontFamilies: (names) => invoke("resolve_font_families", { names }),
  scanSkinCatalog: () => invoke("scan_skin_catalog"),
  // 「制作皮肤」：两条命令都走 client-services 的 `skin::candidate_editor`，失败以 `{code, detail}` 拒绝；装进哪个目录由宿主决定。
  candidateSkinEditor: {
    check: ({ packageId, manifest, files }) =>
      invoke("candidate_skin_check", { packageId, manifest, files }),
    create: ({ packageId, manifest, files }, replace) =>
      invoke("candidate_skin_create", { packageId, manifest, files, replace }),
  },
  listHelpcodeSchemas: () => invoke<CustomHelpcodeSchema[]>("list_helpcode_schemas"),
  resolveTheme: (request) => invoke("resolve_theme", { request }),
  // 两个都是无副作用的计算（host-api 不留存的校验和内置方案的表），桌面宿主都注册了，输入页据此提供自定义双拼的编辑、导入和导出。
  validateShuangpinCustomProfile: (profile) =>
    invoke("validate_shuangpin_custom_profile", { profile }),
  checkToggleHotkey: (request) => invoke("check_toggle_hotkey", { request }),
  builtinShuangpinTable: (name) => invoke("builtin_shuangpin_table", { name }),
  nineKeyShuangpinLayout: (request) =>
    invoke("nine_key_shuangpin_layout", {
      profile: request.profile,
      table: request.table ?? {},
    }),
  readSkinToolbarCss: (id, relative) =>
    relative
      ? invoke("read_skin_stylesheet", { id, relative })
      : invoke("read_skin_toolbar_stylesheet", { id }),
  readSkinImage: (id, relative) => invoke("read_skin_image", { id, relative }),
  readSkinFont: (id, relative) => invoke("read_skin_font", { id, relative }),
  openSkinDirectory: () => invoke("open_skin_directory"),
  load: () => {
    if (!isTauri())
      return Promise.reject(new Error("请通过客户端应用打开设置。浏览器预览不会写入本地配置。"));
    return invoke<Snapshot>("load_preferences");
  },
  save: (expectedRevision, preferences) =>
    invoke<Snapshot>("save_preferences", { expectedRevision, preferences }),
  onPreferencesChanged: (listener) =>
    listen<Snapshot>("preferences-changed", (event) => listener(event.payload)),
  openExternalUrl: (url) => invoke("open_external_url", { url }),
  // The settings window asks for the console's notices when it opens; the host caches the feed for the server's one minute and keeps dismissals per notice id.
  notices: {
    list: () => invoke<AppNotice[]>("notices_list"),
    dismiss: (id) => invoke<void>("notice_dismiss", { id }),
  },
  // 与其他宿主一样，发布列表由 Rust（`msime_client_services::update_check`）读取和比较。
  checkUpdate: (request) => invoke<UpdateCheckResult>("update_check", request),
  openThirdPartyLicenses: () => invoke("open_third_party_licenses"),
  loadMacosShuangpinKeymap: () => invoke<boolean>("load_macos_shuangpin_keymap"),
  copyText: (text) => invoke("copy_text", { text }),
  openScreenKeyboard: () => invoke("open_keyboard_panel"),
  openHandwriting: () => invoke("open_handwriting_panel"),
  listVoiceCaptureDevices: () => invoke("list_voice_capture_devices"),
  openVoice: () => invoke("open_voice_panel"),
  openVocabulary: () => invoke("open_vocabulary_panel"),
  openCloudClipboard: () => invoke("open_cloud_clipboard_panel"),
  cloudClipboardRequest: (action) => invoke("cloud_clipboard_request", { action }),
  openCloudDictionary: () => invoke("open_cloud_dictionary_panel"),
  cloudDictionary: cloudDictionaryClient,
  restartInputMethod: () => invoke("restart_input_method"),
  installInputSource: () => invoke("install_input_source"),
  inputSourceStartup,
  macosInputModes,
  onDeviceTranslation: {
    downloadableLanguages: () => invoke<string[]>("on_device_translation_downloadable_languages"),
    openSettings: () => invoke("open_translation_language_settings"),
  },
  uninstallInputSource: (removeUserData) => invoke("uninstall_input_source", { removeUserData }),
  dataDirectory: {
    status: () => invoke("data_directory_status"),
    pick: () => invoke("pick_data_directory"),
    move: () => invoke("move_data_directory"),
  },
  pickVoiceModelPath: () => invoke("pick_voice_model_path"),
  pickApplication: () => invoke("pick_application"),
  applicationNames: (ids) => invoke("application_names", { ids }),
  localVoiceModels,
  windowControl: async (action) => {
    const window = getCurrentWindow();
    if (action === "minimize") return window.minimize();
    if (action === "close") return window.close();
    if (action === "restore") return window.unmaximize();
    return window.maximize();
  },
  beginWindowDrag: () => getCurrentWindow().startDragging(),
  resizeWindow: (edge) =>
    getCurrentWindow().startResizeDragging(
      {
        n: "North",
        s: "South",
        e: "East",
        w: "West",
        ne: "NorthEast",
        nw: "NorthWest",
        se: "SouthEast",
        sw: "SouthWest",
      }[edge] as Parameters<ReturnType<typeof getCurrentWindow>["startResizeDragging"]>[0],
    ),
  onWindowStateChanged: (listener, onError) =>
    subscribeWindowState(getCurrentWindow(), listener, onError),
  clipboard: clipboardClient,
  dictionary,
  loadDefaultPreferences: () => invoke<Preferences>("restored_default_preferences"),
  /* mobile host services are injected after host_capabilities resolves */
};
const panelClients = createDesktopPanelClients(client, cloudDictionaryClient, clipboardClient);
const panel = new URLSearchParams(window.location.search).get("panel");
// 桌面的手写面板在宿主列出手写模型时第一次打开就下载它，下载失败时就地设置下载镜像。客户端固定为模块级对象，避免每次渲染换一个 client 让面板重置识别队列。
const desktopHandwritingClient: PanelClient = {
  ...panelClients.handwriting,
  resourcePacks,
  modelMirror: savedModelMirror(client),
};
function DesktopHandwritingSurface({ theme }: { theme: "dark" | "light" }) {
  const platform = useHostPlatform(client.host);
  return (
    <DesktopHandwriting
      client={
        platform === "macos" || platform === "windows" || platform === "linux"
          ? desktopHandwritingClient
          : panelClients.handwriting
      }
      theme={theme}
      platform={platform}
    />
  );
}

/** 独立的表情和本地剪贴板窗口：等宿主报告平台后再挂面板，免得面板先按一种平台装好客户端、随后又整个换掉。 */
function DesktopEmojiSurface({
  theme,
  initialPage,
}: {
  theme: "dark" | "light";
  initialPage?: "home" | "clipboard";
}) {
  const host = usePanelHost(client.host);
  return host.resolved ? (
    <DesktopEmojiPanel
      client={panelClients.emoji}
      theme={theme}
      initialPage={initialPage}
      hostPlatform={host.host?.platform}
    />
  ) : (
    <StatusMessage role="status">正在连接面板…</StatusMessage>
  );
}

// The host reports what it supports; outside Tauri (the browser preview) there is no host.
async function discoverHostCapabilities(): Promise<HostCapabilities | null> {
  if (!isTauri()) return null;
  return await invoke<HostCapabilities>("host_capabilities");
}

function DesktopSettings() {
  const [settingsClient, setSettingsClient] = useState<SettingsClient | null>(null);
  const [bootstrapRequired, setBootstrapRequired] = useState<boolean | null>(null);
  const [linuxSetup, setLinuxSetup] = useState<LinuxSetupStatus | null>(null);
  // 宿主是 Linux（被叫到前台时要重看词库是否过期），以及用户在本进程里已对「词库需要更新」点过「稍后」。
  const linuxHost = useRef(false);
  const linuxUpdateDeferred = useRef(false);
  const [macosInstall, setMacosInstall] = useState(false);
  const [replayOnboarding, setReplayOnboarding] = useState(false);
  // 首启引导准备资源之后重新读到的版本：第一次启动时 HostOptions 由这一步写下，发现宿主能力时还读不到版本。
  const [preparedEdition, setPreparedEdition] = useState<HostCapabilities["edition"]>();
  const [mobilePanel, setMobilePanel] = useState<
    | "voice"
    | "emoji"
    | "clipboard"
    | "cloud-clipboard"
    | "cloud-dictionary"
    | "cloud-dictionary-catalog"
    | "cloud-candidates"
    | "cloud-dictionary-files"
    | "cloud-dictionary-apply"
    | null
  >(null);
  const mobilePanelRef = useRef(mobilePanel);
  const [initialPage, setInitialPage] = useState<string | undefined>();
  // A section requested while the page is already open. It navigates the mounted page rather than
  // remounting it, which used to drop an unsaved draft; the nonce makes a repeated request count.
  const [settingsRoute, setSettingsRoute] = useState<{ page: string; nonce: number }>();
  const requestSettingsPage = useCallback((page: string) => {
    setInitialPage(page);
    setSettingsRoute((current) => ({ page, nonce: (current?.nonce ?? 0) + 1 }));
  }, []);
  useEffect(() => {
    mobilePanelRef.current = mobilePanel;
  }, [mobilePanel]);
  const navigateMobilePanel = (next: NonNullable<typeof mobilePanel>, replace = false) => {
    if (typeof window !== "undefined") {
      const current = window.history.state;
      const state = createMobileSettingsHistoryState(current, { panel: next });
      if (replace) window.history.replaceState(state, "");
      else window.history.pushState(state, "");
    }
    setMobilePanel(next);
  };
  const closeMobilePanel = useCallback(() => {
    if (
      typeof window !== "undefined" &&
      window.history.state?.msimeSettings === true &&
      window.history.state?.panel
    ) {
      window.history.back();
    } else {
      setMobilePanel(null);
    }
  }, []);
  // The mobile panels restart their catalog, clipboard subscription or recording whenever their
  // client changes, so these are built once rather than on every render of this component.
  const closeMobilePanelAsync = useCallback(async () => closeMobilePanel(), [closeMobilePanel]);
  const mobileEmojiClient = useMemo(
    () => ({
      ...panelClients.emoji,
      close: closeMobilePanelAsync,
      rememberInputTarget: undefined,
      sendText: undefined,
    }),
    [closeMobilePanelAsync],
  );
  const iosHost = settingsClient?.host?.platform === "ios";
  const mobileVoiceClient = useMemo(
    () => ({
      ...panelClients.voice,
      close: closeMobilePanelAsync,
      rememberInputTarget: undefined,
      ...(iosHost
        ? {
            description:
              "iOS App 负责录音和识别；识别结果不会直接写入键盘扩展，确认提交后会保存为待插入的语音结果。",
            submitNotice: "已发送到本机键盘。返回目标 App，打开键盘“更多 → 语音结果”，确认后插入。",
          }
        : {}),
    }),
    [closeMobilePanelAsync, iosHost],
  );
  useMobilePopState(true, (event) => {
    const state = event.state;
    if (state?.msimeSettings === true && typeof state.panel === "string") {
      setMobilePanel(state.panel as NonNullable<typeof mobilePanel>);
    } else if (mobilePanelRef.current !== null) {
      setMobilePanel(null);
    }
  });
  useEffect(() => {
    const onNativePanel = (event: Event) => {
      const panel = (event as CustomEvent<unknown>).detail;
      if (
        typeof panel === "string" &&
        [
          "voice",
          "emoji",
          "clipboard",
          "cloud-clipboard",
          "cloud-dictionary",
          "cloud-dictionary-catalog",
          "cloud-candidates",
          "cloud-dictionary-files",
          "cloud-dictionary-apply",
        ].includes(panel)
      ) {
        setMobilePanel(panel as NonNullable<typeof mobilePanel>);
      }
    };
    const onNativeSettingsPage = (event: Event) => {
      const page = (event as CustomEvent<unknown>).detail;
      if (
        typeof page === "string" &&
        ["home", "appearance", "dictionary", "account", "about", "help", "feedback"].includes(page)
      ) {
        requestSettingsPage(page);
        setMobilePanel(null);
      }
    };
    const stopNativePanel = subscribeToWindowEvent("msime-mobile-panel", onNativePanel);
    const stopNativeSettingsPage = subscribeToWindowEvent(
      "msime-settings-page",
      onNativeSettingsPage,
    );
    return () => {
      stopNativePanel();
      stopNativeSettingsPage();
    };
  }, []);
  // The host menu entry that started this window names a section; resolve it
  // before mounting so the page never opens on one and then jumps.
  useEffect(() => {
    let active = true;
    const disposeSubscription = isTauri()
      ? trackAsyncSubscription(
          listen<string>("settings-route", (event) => {
            if (!active) return;
            // 未指定分区的入口只将窗口带到前台，保留当前页面。
            if (event.payload) requestSettingsPage(event.payload);
            // 设置进程可能早于宿主留下词库过期标记就在运行（关窗后还常驻十分钟），被叫到前台时（包括点「词库需要更新」通知的「打开设置」）再看一次，与启动时一样先打开「立即更新」页。用户在本进程里点过「稍后」就不再打断。
            if (linuxHost.current && !linuxUpdateDeferred.current)
              void invoke<LinuxSetupStatus>("linux_setup_status")
                .then((status) => {
                  if (active && status.prepared && status.dictionaryOutdated)
                    setLinuxSetup((current) => current ?? status);
                })
                .catch(() => undefined);
          }),
          () => active,
        )
      : undefined;
    const requested = isTauri()
      ? invoke<string | null>("initial_settings_page").catch(() => null)
      : Promise.resolve(null);
    void Promise.all([
      discoverFontReader(isTauri(), invoke),
      requested,
      discoverHostCapabilities(),
    ]).then(async ([reader, page, host]) => {
      if (!active) return;
      const android = host?.platform === "android";
      const ios = host?.platform === "ios";
      const ready = android
        ? await invoke<boolean>("android_bootstrap_status").catch(() => false)
        : ios
          ? await invoke<boolean>("ios_onboarding_status").catch(() => false)
          : true;
      // Linux prepares its runtime options from the first-run page instead of refusing to start without them.
      const linuxSetupStatus =
        host?.platform === "linux"
          ? await invoke<LinuxSetupStatus>("linux_setup_status").catch(() => null)
          : null;
      // The window is already at the install window's size when this asks; see `first_install_window_pending`.
      const macosInstallPending =
        host?.platform === "macos"
          ? await invoke<boolean>("first_install_window_pending").catch(() => false)
          : false;
      const applicationUpdater =
        host?.platform === "macos"
          ? await invoke<boolean>("application_update_supported").catch(() => false)
          : false;
      if (!active) return;
      linuxHost.current = host?.platform === "linux";
      // 还没配置，或宿主报告词库过期时先打开首次配置（更新）页；宿主已在启动时把窗口变成它的小窗口。其余情况把窗口换回设置的大小（宿主没变过窗口时这一步什么也不做）。
      if (linuxSetupStatus && (!linuxSetupStatus.prepared || linuxSetupStatus.dictionaryOutdated))
        setLinuxSetup(linuxSetupStatus);
      else if (host?.platform === "linux")
        void invoke("leave_linux_setup_window").catch(() => undefined);
      setMacosInstall(macosInstallPending);
      setBootstrapRequired((android || ios) && !ready);
      setInitialPage(page ?? undefined);
      // 应用启动时补传一次打字统计；没打开上传、没登录或十分钟内传过时宿主什么也不做，失败也不打扰用户。
      if (host?.typing_statistics && isDesktopAccountHost(host.platform))
        void typingStatisticsUpload.sync().catch(() => undefined);
      const hosted: SettingsClient = host
        ? {
            ...client,
            host,
            ...(applicationUpdater
              ? {
                  checkForApplicationUpdate: () => invoke<void>("application_update_check"),
                }
              : {}),
            // Linux 设置应用每天在后台检查一次更新并发通知（不在应用里安装），关于页先显示它找到的新版本。
            ...(host.platform === "linux"
              ? {
                  pendingUpdateCheck: () =>
                    invoke<UpdateCheckResult | null>("pending_update_check"),
                }
              : {}),
            dictionary: isMobileHost(host.platform) ? mobileDictionary : dictionary,
            ...(!isMobileHost(host.platform) && client.cloudDictionary
              ? {
                  cloudDictionary: {
                    ...createDesktopCloudDictionaryClient(
                      client.cloudDictionary,
                      host.platform,
                      mobileDictionary,
                      saveCloudDictionaryExport,
                    ),
                    ...(host.platform === "macos"
                      ? {
                          snapshotNative: false,
                          snapshotAppliesInPanel: true,
                          chooseSnapshotRestore: undefined,
                          saveExport: async (name: string, contents: string) => {
                            await invoke<string>("save_export", { name, contents });
                          },
                        }
                      : {}),
                  },
                }
              : {}),
            // Windows 和 macOS 在各自的原生候选控制器里查离线释义，所以释义设置在两边都生效。读音也是这两个宿主画在释义后面（日文罗马音在 macOS 来自系统分词器，在 Windows 来自系统的微软日语输入法）；其他宿主不动这个值。
            candidatePronunciation: host.platform === "macos" || host.platform === "windows",
            candidateEnglishGloss:
              host.platform === "linux" ||
              host.platform === "android" ||
              host.platform === "windows" ||
              host.platform === "macos" ||
              host.platform === "ios",
            // A file manager is only reachable on the desktop hosts; iOS and Android get the same
            // page without the button rather than one that fails when pressed.
            // macOS 也不给「打开数据目录」：统计文件就在偏好目录里，和「维护与诊断 › 在 Finder 中显示」打开的是同一个文件夹（Tauri 的 `open_typing_statistics_directory` 与 `open_diagnostic_log_directory` 在 macOS 上都指向 `preferences_directory`）。Linux 和 Windows 上统计页这一个按钮是唯一入口，照旧保留。
            ...(host.typing_statistics
              ? {
                  typingStatistics: {
                    ...typingStatistics,
                    ...(isMobileHost(host.platform) || host.platform === "macos"
                      ? {}
                      : { openDirectory: () => invoke<void>("open_typing_statistics_directory") }),
                    // 「上传到云端」只接在三个桌面宿主的账号会话上；移动端和鸿蒙还没有。
                    ...(isDesktopAccountHost(host.platform)
                      ? { cloudUpload: typingStatisticsUpload }
                      : {}),
                  },
                }
              : {}),
            ...(host.platform === "macos" ||
            host.platform === "linux" ||
            host.platform === "windows"
              ? { resourcePacks }
              : {}),
            // The macOS input method writes diagnostic.log under Application Support, which the Finder hides; the host reveals it rather than asking the user to navigate there. Linux 的两个宿主把它写进数据目录（通常是隐藏的 ~/.config 下），宿主同样在文件管理器里选中它。
            ...(host.platform === "macos" || host.platform === "linux"
              ? { openDiagnosticLogDirectory: () => invoke<void>("open_diagnostic_log_directory") }
              : {}),
            // A host that has not wired 背单词 hides the page rather than offering buttons whose every press would fail.
            ...(host.vocabulary_review ? { vocabularyReview } : {}),
            // This shell registers no download handler, and the macOS WKWebView cancels every download link without one, so the host writes the export into Downloads itself and the page can say where the file went. Linux runs the same shell and takes the same path; Windows' WebView2 and the mobile webviews keep the download link.
            ...(host.platform === "macos" || host.platform === "linux"
              ? {
                  saveExport: (name: string, contents: string) =>
                    invoke<string>("save_export", { name, contents }),
                }
              : {}),
            // The pack store and its import picker are the desktop shells' own; the mobile hosts keep their keyboard feedback settings instead.
            ...(host.platform === "macos" ||
            host.platform === "linux" ||
            host.platform === "windows"
              ? { plugins }
              : {}),
            // 只有三个桌面宿主真正能清除学习数据：Android 的个人词库对 `reset` 一律报错，iOS 的 `reset` 不经过键盘扩展的个人词库，走的是 App 自己的引擎数据，不能保证清掉键盘扩展学到的内容，所以移动端不提供 `resetLearnedData`，设置页也就不显示这个按钮。
            ...(host.platform === "macos" ||
            host.platform === "linux" ||
            host.platform === "windows"
              ? {
                  resetLearnedData: () =>
                    invoke("dictionary_request", { action: { operation: "reset" } }).then(
                      () => undefined,
                    ),
                }
              : {}),
            // 「从文件导入」要用宿主的打开对话框选文件，只有三个桌面宿主注册了对话框插件。
            ...(host.platform === "macos" ||
            host.platform === "linux" ||
            host.platform === "windows"
              ? {
                  localVoiceModels: {
                    ...localVoiceModels,
                    import: (id: string) =>
                      invoke<string | null>("voice_local_model_import", { id }),
                  },
                }
              : {}),
            // msime-mcp is packaged beside the settings app on the three desktop hosts only.
            ...(host.platform === "macos" ||
            host.platform === "linux" ||
            host.platform === "windows"
              ? {
                  mcpServerStatus: () => invoke<McpServerStatus>("mcp_server_status"),
                  installMcpClient: (
                    client: McpClientId,
                    replace: boolean,
                    flags: readonly McpFlag[],
                  ) => invoke<McpInstallOutcome>("install_mcp_client", { client, replace, flags }),
                }
              : {}),
            ...(host.fuzzy_pinyin ? { fuzzyPinyin: true } : {}),
            // iOS has no recover_preferences command: its keyboard mirrors the AI settings natively and only a save keeps that mirror in step.
            ...(host.platform !== "ios"
              ? {
                  recoverPreferences: () => invoke<PreferencesRecovery>("recover_preferences"),
                }
              : {}),
            ...(host.platform === "macos" ||
            host.platform === "linux" ||
            host.platform === "windows"
              ? {
                  openPreferencesDirectory: () => invoke<void>("open_preferences_directory"),
                }
              : {}),
            ...(host.platform === "ios" || host.platform === "android"
              ? createMobileHostServices(host.platform, {
                  invoke,
                  listen,
                  navigateVoice: () => navigateMobilePanel("voice"),
                })
              : {}),
            ...(host.platform === "ios" || host.platform === "android"
              ? {
                  openSystemKeyboardSettings: () =>
                    invoke(
                      host.platform === "ios"
                        ? "open_system_keyboard_settings"
                        : "android_open_input_method_settings",
                    ),
                }
              : {}),
            // 云剪贴板可以改走自建中继：macOS 与 Linux 的设置页和云剪贴板面板都在这个 Tauri 进程里，按偏好选路（`cloud_clipboard_request`）；macOS 输入法菜单启动的面板、原生云剪贴板窗口和表情面板的「发到云剪贴板」也按同一个 C 入口选路。Windows 的设置窗口是原生 WinUI，那一组在它自己的设置页里。
            ...(host.platform === "macos" || host.platform === "linux"
              ? { cloudClipboardRelay: true }
              : {}),
            ...(host.platform === "linux"
              ? {
                  customTouchKeyboardSkins: true,
                  customSkinLibrary: {
                    load: () => invoke("load_custom_skin_library"),
                    mutate: (action) => invoke("mutate_custom_skin_library", { action }),
                  },
                  testApiCredential: (
                    service: ApiCredentialTestService,
                    config: Record<string, unknown>,
                  ) => invoke<ApiCredentialTestResult>("test_api_credential", { service, config }),
                  providerCredentials: {
                    status: () => invoke<ProviderCredentialStatus>("provider_credentials_status"),
                    saveAi: ({ provider, endpoint, model, token }) =>
                      invoke<ProviderCredentialStatus>("save_ai_provider_credential", {
                        provider,
                        endpoint,
                        model,
                        token,
                      }),
                    clearAi: (provider) =>
                      invoke<ProviderCredentialStatus>("clear_ai_provider_credential", {
                        provider,
                      }),
                    saveTencent: ({ secretId, secretKey, region }) =>
                      invoke<ProviderCredentialStatus>("save_tencent_provider_credential", {
                        secretId,
                        secretKey,
                        region,
                      }),
                    clearTencent: () =>
                      invoke<ProviderCredentialStatus>("clear_tencent_provider_credential"),
                    saveVoice: (credential) =>
                      invoke<VoiceCredentialSaveResult>(
                        "save_voice_provider_credential",
                        credential,
                      ),
                    clearVoice: (kind, provider) =>
                      invoke<VoiceCredentialSaveResult>("clear_voice_provider_credential", {
                        kind,
                        provider,
                      }),
                  } satisfies ProviderCredentialClient,
                }
              : {}),
            ...(host.platform === "windows" || host.platform === "macos"
              ? {
                  testApiCredential: testDesktopApiCredential,
                  aiAssistant: {
                    fetchModels: ({ endpoint, token }) =>
                      invoke<string[]>("ai_models", { endpoint, token }),
                    test: ({ endpoint, model, prompt, token, text }) =>
                      invoke<string>("ai_test", { endpoint, model, prompt, token, text }),
                  },
                }
              : {}),
            // Same two commands, and the host resolves them through the provider
            // service that holds the credential. The token is deliberately not
            // passed: it is not in this process on this platform.
            ...(host.platform === "linux"
              ? {
                  aiAssistant: {
                    fetchModels: ({ endpoint, provider }) =>
                      invoke<string[]>("ai_models", { endpoint, provider }),
                    test: ({ endpoint, model, prompt, text, provider }) =>
                      invoke<string>("ai_test", { endpoint, model, prompt, text, provider }),
                  },
                }
              : {}),
            ...(host.platform === "windows" ||
            host.platform === "macos" ||
            host.platform === "linux"
              ? {
                  account: {
                    ...createAccountCommandClient(invoke),
                    googleLogin: () => invoke("account_google_login"),
                    googleCancel: () => invoke("account_google_cancel"),
                    avatar: () => invoke<string | null>("account_avatar"),
                    chooseAvatar: () => invoke<AccountProfile | null>("account_choose_avatar"),
                    removeAvatar: () => invoke<AccountProfile>("account_remove_avatar"),
                    qqLinks,
                    // 设置同步接在 Windows 和 Linux 的账号会话上；macOS 由输入法自己的账号窗口同步它的原生设置。
                    ...(host.platform === "windows" || host.platform === "linux"
                      ? { settingsSync: createDesktopSettingsSync(invoke) }
                      : {}),
                  } satisfies AccountClient,
                  communityCandidateSkins: createDesktopCandidateSkinCommunity(invoke),
                  communityPlugins: createDesktopPluginCommunity(invoke),
                  // 社区词包与回复模板同理，在 Windows 和 Linux 的设置应用里提供。
                  ...(host.platform === "windows" || host.platform === "linux"
                    ? { communityResources: createDesktopCommunityResources(invoke) }
                    : {}),
                }
              : {}),
            ...(host.platform === "ios"
              ? {
                  testApiCredential: (
                    service: ApiCredentialTestService,
                    config: Record<string, unknown>,
                  ) => invoke<ApiCredentialTestResult>("test_api_credential", { service, config }),
                }
              : {}),
          }
        : client;
      const mobileHosted =
        host?.platform === "android"
          ? {
              ...hosted,
              home: {
                ...hosted.home,
                openEmojiPanel: async () => navigateMobilePanel("emoji"),
                openClipboardPanel: async () => navigateMobilePanel("clipboard"),
              },
              openCloudClipboard: async () => navigateMobilePanel("cloud-clipboard"),
              openCloudDictionary: async () => navigateMobilePanel("cloud-dictionary"),
            }
          : host?.platform === "ios"
            ? {
                ...hosted,
                openCloudClipboard: async () => navigateMobilePanel("cloud-clipboard"),
                openCloudDictionary: async () => navigateMobilePanel("cloud-dictionary"),
              }
            : hosted;
      setSettingsClient(reader ? { ...mobileHosted, listFontFamilies: reader } : mobileHosted);
    });
    return () => {
      active = false;
      disposeSubscription?.();
    };
  }, []);
  const onboardingPlatform = settingsClient?.host?.platform;
  const onboardingActions: OnboardingActions = {
    platform: onboardingPlatform === "ios" ? "ios" : "android",
    prepareResources:
      onboardingPlatform === "android" || !onboardingPlatform
        ? async () => {
            await invoke("android_prepare_bootstrap");
            setPreparedEdition((await discoverHostCapabilities())?.edition);
          }
        : async () => undefined,
    openSystemKeyboardSettings:
      onboardingPlatform === "ios"
        ? () => invoke("open_system_keyboard_settings").then(() => undefined)
        : () => invoke("android_open_input_method_settings").then(() => undefined),
    showInputMethodPicker:
      onboardingPlatform === "android" || !onboardingPlatform
        ? () => invoke("android_show_input_method_picker").then(() => undefined)
        : async () => undefined,
  };
  const completeOnboarding = async (scheme: OnboardingInputScheme, choices: OnboardingChoices) => {
    const snapshot = await client.load();
    await client.save(snapshot.revision, completeOnboardingPreferences(snapshot, scheme, choices));
    if (onboardingPlatform === "ios") await invoke("ios_onboarding_complete");
    // Signing in happens on 我的, so the flow's 登录 lands there once the settings page mounts.
    if (choices.openAccount) requestSettingsPage("account");
    setBootstrapRequired(false);
    setReplayOnboarding(false);
  };
  const skipOnboarding = async () => {
    if (onboardingPlatform === "ios") await invoke("ios_onboarding_complete");
    setBootstrapRequired(false);
    setReplayOnboarding(false);
  };
  if (linuxSetup)
    return (
      <LinuxSetupPage
        status={linuxSetup}
        client={linuxSetupClient}
        edition={settingsClient?.host?.edition}
        onComplete={() => {
          if (linuxSetup.prepared && linuxSetup.dictionaryOutdated)
            linuxUpdateDeferred.current = true;
          void invoke("leave_linux_setup_window")
            .catch(() => undefined)
            .then(() => setLinuxSetup(null));
        }}
      />
    );
  if (macosInstall)
    return (
      <MacosInstallPage
        client={macosInstallClient}
        onComplete={() => {
          void invoke("leave_first_install_window")
            .catch(() => undefined)
            .then(() => setMacosInstall(false));
        }}
      />
    );
  // Mount once after discovery: replacing the client later would reload draft preferences.
  if (bootstrapRequired || replayOnboarding)
    return (
      <WelcomeFlowPage
        actions={onboardingActions}
        onComplete={completeOnboarding}
        // Android can leave too: the flow prepares the built-in dictionaries before it lets go, and those are what the next launch checks.
        onSkip={skipOnboarding}
        // The splash belongs to a first launch; replaying the flow from settings skips it.
        splash={Boolean(bootstrapRequired) && !replayOnboarding}
        edition={preparedEdition ?? settingsClient?.host?.edition}
      />
    );
  if (!settingsClient)
    return (
      <SettingsStartupPage
        onClose={
          isTauri()
            ? () => {
                void getCurrentWindow().close();
              }
            : undefined
        }
      />
    );
  const cloudDictionary = createDesktopCloudDictionaryClient(
    panelClients.cloudDictionary,
    settingsClient.host?.platform,
    mobileDictionary,
    saveCloudDictionaryExport,
  );
  if (mobilePanel === "voice") {
    return <VoicePanel client={mobileVoiceClient} theme="light" />;
  }
  if (mobilePanel === "emoji" || mobilePanel === "clipboard") {
    return (
      <DesktopPanelTheme preferences={settingsClient} surface="emoji">
        {(theme) => (
          <DesktopEmojiPanel
            theme={theme}
            initialPage={mobilePanel === "clipboard" ? "clipboard" : "home"}
            client={mobileEmojiClient}
            close={closeMobilePanelAsync}
          />
        )}
      </DesktopPanelTheme>
    );
  }
  if (mobilePanel === "cloud-clipboard") {
    return (
      <CloudClipboardPanel
        client={{
          ...panelClients.cloudClipboard,
          rememberInputTarget: undefined,
          sendText: undefined,
          close: async () => closeMobilePanel(),
        }}
      />
    );
  }
  if (mobilePanel === "cloud-dictionary") {
    return (
      <CloudDictionaryPanel
        client={{
          ...cloudDictionary,
          openCatalog: async () => navigateMobilePanel("cloud-dictionary-catalog"),
          openCandidates: async () => navigateMobilePanel("cloud-candidates"),
          openFiles: async () => navigateMobilePanel("cloud-dictionary-files"),
          openApply: async () => navigateMobilePanel("cloud-dictionary-apply"),
          close: async () => closeMobilePanel(),
        }}
      />
    );
  }
  if (mobilePanel === "cloud-dictionary-catalog") {
    return (
      <CloudDictionaryCatalogPanel
        client={{
          ...cloudDictionary,
          back: async () => navigateMobilePanel("cloud-dictionary", true),
          close: async () => closeMobilePanel(),
        }}
      />
    );
  }
  if (mobilePanel === "cloud-candidates") {
    return (
      <CloudCandidatesPanel
        client={{
          ...cloudDictionary,
          back: async () => navigateMobilePanel("cloud-dictionary", true),
          close: async () => closeMobilePanel(),
        }}
      />
    );
  }
  if (mobilePanel === "cloud-dictionary-files") {
    return (
      <CloudDictionaryFilesPanel
        client={{
          ...cloudDictionary,
          back: async () => navigateMobilePanel("cloud-dictionary", true),
          close: async () => closeMobilePanel(),
        }}
      />
    );
  }
  if (mobilePanel === "cloud-dictionary-apply") {
    return (
      <CloudDictionaryApplyPanel
        client={{
          ...cloudDictionary,
          back: async () => navigateMobilePanel("cloud-dictionary", true),
          close: async () => closeMobilePanel(),
        }}
      />
    );
  }
  return (
    <SettingsPage
      client={settingsClient}
      initialPage={initialPage}
      route={settingsRoute}
      onReplayOnboarding={() => setReplayOnboarding(true)}
    />
  );
}
function DesktopCloudDictionarySurface() {
  const [host, setHost] = useState<HostCapabilities | null | undefined>(undefined);
  useEffect(() => {
    let active = true;
    void discoverHostCapabilities().then((value) => {
      if (active) setHost(value);
    });
    return () => {
      active = false;
    };
  }, []);
  if (host === undefined) return <StatusMessage role="status">正在连接云词库…</StatusMessage>;
  const cloudDictionary = createDesktopCloudDictionaryClient(
    panelClients.cloudDictionary,
    host?.platform,
    mobileDictionary,
    saveCloudDictionaryExport,
  );
  return <DesktopCloudDictionary client={cloudDictionary} />;
}

const content =
  panel === "keyboard" ? (
    <DesktopKeyboard client={panelClients.keyboard} preferences={client} />
  ) : panel === "handwriting" ? (
    <DesktopPanelTheme preferences={client} surface="handwriting">
      {(theme) => <DesktopHandwritingSurface theme={theme} />}
    </DesktopPanelTheme>
  ) : panel === "voice" ? (
    <DesktopPanelTheme preferences={client} surface="voice">
      {(theme) => <VoicePanel client={panelClients.voice} theme={theme} />}
    </DesktopPanelTheme>
  ) : panel === "cloud-clipboard" ? (
    <CloudClipboardPanel client={panelClients.cloudClipboard} />
  ) : panel === "cloud-dictionary" ? (
    <DesktopCloudDictionarySurface />
  ) : panel === "clipboard" ? (
    <DesktopPanelTheme preferences={client} surface="emoji">
      {(theme) => <DesktopEmojiSurface theme={theme} initialPage="clipboard" />}
    </DesktopPanelTheme>
  ) : panel === "emoji" ? (
    <DesktopPanelTheme preferences={client} surface="emoji">
      {(theme) => <DesktopEmojiSurface theme={theme} />}
    </DesktopPanelTheme>
  ) : panel === "vocabulary" ? (
    <VocabularyReviewPanel client={vocabularyReview} />
  ) : (
    <DesktopSettings />
  );
createRoot(document.getElementById("root")!).render(<StrictMode>{content}</StrictMode>);
