import { createVoiceRecognitionClient } from "./voice/voice-recognition-client";
import {
  StrictMode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createRoot } from "react-dom/client";
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
  EmojiPanel,
  HandwritingPanel,
  VoicePanel,
  completeOnboardingPreferences,
  usePreferencesSnapshot,
  SettingsPage,
  SettingsStartupPage,
  WelcomeFlowPage,
  LinuxSetupPage,
  MacosInstallPage,
  savedModelMirror,
  useCandidatePreviewTheme,
  type AccountClient,
  type AccountProfile,
  type ApiCredentialTestResult,
  type ApiCredentialTestService,
  type ClipboardHistoryEntry,
  type CloudClipboardAction,
  type CloudClipboardPanelClient,
  type CloudDictionaryAction,
  type CloudDictionaryEntry,
  type CloudDictionaryPanelClient,
  type EmojiCatalogGroup,
  type EmojiPanelClient,
  type CustomHelpcodeSchema,
  type HostCapabilities,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
  type VoiceCredentialSaveResult,
  type TypingStatisticsClient,
  VocabularyReviewPanel,
  type VocabularyReviewClient,
  type PanelClient,
  type VoicePanelClient,
  type Preferences,
  type PreferencesRecovery,
  type AppNotice,
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
  type LinuxSetupStatus,
  type McpClientId,
  type McpFlag,
  type McpInstallOutcome,
  type McpServerStatus,
  type LocalVoiceModelList,
  type LocalVoiceModelProgress,
  type MentionEntry,
  type ResourcePackClient,
  type ResourcePackStatus,
  type PluginCatalogResult,
  type PluginClient,
  type PluginPackage,
  UNBATCHED_DICTIONARY_FILE_BYTES,
  StatusMessage,
} from "@msime/ui";
import "@msime/ui/styles.css";
import { subscribeWindowState } from "./input/window-state";
import { discoverFontReader } from "./candidate/system-font-client";
import { DesktopKeyboard, useHostPlatform } from "./input/desktop-keyboard";
import { DesktopCloudDictionary } from "./dictionary/desktop-cloud-dictionary";
import { testDesktopApiCredential } from "./account/credential-test-client";
import { cloudDictionaryCapabilities, isMobileHost } from "./input/mobile-host-capabilities";
import { createMobileHostServices } from "./core/mobile-host-services";
import {
  createDesktopCandidateSkinCommunity,
  createDesktopPluginCommunity,
} from "./core/desktop-host-services";

const dictionary: DictionaryClient = {
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
};
const mobileDictionary: DictionaryClient = {
  ...dictionary,
  // The mobile bridge sends an import to the host in one request rather than in batches.
  maxImportFileBytes: UNBATCHED_DICTIONARY_FILE_BYTES,
  importPersonal: (text: string, request_id: string) =>
    invoke("dictionary_request", { action: { operation: "import_personal", text, request_id } }),
};

async function downloadCloudEntryToLocal(
  entry: CloudDictionaryEntry,
  dictionaryClient: DictionaryClient,
): Promise<void> {
  if (!dictionaryClient.importPersonal)
    throw new Error("personal dictionary import is unavailable");
  const text = JSON.stringify({
    format: "msime-personal-dictionary",
    version: 1,
    entries: [
      {
        kind: entry.kind === "quick" ? "quickPhrase" : entry.kind,
        key: entry.code,
        value: entry.word,
        weight: entry.weight,
      },
    ],
  });
  await dictionaryClient.importPersonal(text, `ui-cloud-download-${Date.now()}`);
}
const linuxSetupClient: LinuxSetupClient = {
  run: async ({ download, cloudCandidates }, onLine) => {
    const unlisten = await listen<LinuxSetupLine>("linux-setup-output", (event) =>
      onLine(event.payload),
    );
    try {
      return await invoke<LinuxSetupStatus>("run_linux_setup", { download, cloudCandidates });
    } finally {
      unlisten();
    }
  },
};
// The host resolves the plugins directory and the built-in sound packs, and shows its own picker for an import; the page never names a path.
const plugins: PluginClient = {
  catalog: () => invoke<PluginCatalogResult>("plugin_catalog"),
  importPack: (source) => invoke<PluginPackage | null>("import_plugin_pack", { source }),
  remove: (kind, id) => invoke<void>("remove_plugin_pack", { kind, id }),
  loadMentions: () => invoke<MentionEntry[]>("load_plugin_mentions"),
  saveMentions: (entries) => invoke<void>("save_plugin_mentions", { entries }),
};
const typingStatistics: TypingStatisticsClient = {
  load: () => invoke("load_typing_statistics"),
  setEnabled: (enabled: boolean) => invoke("set_typing_statistics_enabled", { enabled }),
  setRetention: (retention: string) => invoke("set_typing_statistics_retention", { retention }),
  reset: () => invoke("reset_typing_statistics"),
};
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
  list: () => invoke<ResourcePackStatus[]>("resource_packs"),
  install: (id) => invoke<string>("resource_pack_install", { id }),
  cancel: (id) => invoke<boolean>("resource_pack_cancel", { id }),
  onProgress: (listener) =>
    listen<LocalVoiceModelProgress>("resource-pack-progress", (event) => listener(event.payload)),
};
const macosInstallClient: MacosInstallClient = {
  install: () => invoke("run_first_input_source_install"),
};
const client: SettingsClient = {
  readAppVersion: getVersion,
  resolveFontFamilies: (names) => invoke("resolve_font_families", { names }),
  scanSkinCatalog: () => invoke("scan_skin_catalog"),
  listHelpcodeSchemas: () => invoke<CustomHelpcodeSchema[]>("list_helpcode_schemas"),
  resolveTheme: (request) => invoke("resolve_theme", { request }),
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
  openThirdPartyLicenses: () => invoke("open_third_party_licenses"),
  loadMacosShuangpinKeymap: () => invoke<boolean>("load_macos_shuangpin_keymap"),
  saveMacosShuangpinKeymap: (enabled) => invoke("save_macos_shuangpin_keymap", { enabled }),
  loadMacosWubiAutoCommitUnique: () => invoke<boolean>("load_macos_wubi_auto_commit_unique"),
  saveMacosWubiAutoCommitUnique: (enabled) =>
    invoke("save_macos_wubi_auto_commit_unique", { enabled }),
  copyText: (text) => invoke("copy_text", { text }),
  openScreenKeyboard: () => invoke("open_keyboard_panel"),
  openHandwriting: () => invoke("open_handwriting_panel"),
  listVoiceCaptureDevices: () => invoke("list_voice_capture_devices"),
  openVoice: () => invoke("open_voice_panel"),
  openVocabulary: () => invoke("open_vocabulary_panel"),
  openCloudClipboard: () => invoke("open_cloud_clipboard_panel"),
  cloudClipboardRequest: (action) => invoke("cloud_clipboard_request", { action }),
  openCloudDictionary: () => invoke("open_cloud_dictionary_panel"),
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
  localVoiceModels: {
    list: () => invoke<LocalVoiceModelList>("voice_local_models"),
    install: (id) => invoke<string>("voice_local_model_install", { id }),
    cancel: (id) => invoke<boolean>("voice_local_model_cancel", { id }),
    remove: (id) => invoke<void>("voice_local_model_remove", { id }),
    onProgress: (listener) =>
      listen<LocalVoiceModelProgress>("voice-local-model-progress", (event) =>
        listener(event.payload),
      ),
  },
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
  clipboard: {
    clear: () => invoke("clear_clipboard_history"),
    list: () => invoke<ClipboardHistoryEntry[]>("list_clipboard_history"),
    sync: () => invoke<ClipboardHistoryEntry[]>("sync_clipboard_history"),
    copy: (text) => invoke("copy_text", { text }),
    remove: (text) => invoke("remove_clipboard_history", { text }),
    setPinned: (text, pinned) => invoke("set_clipboard_history_pinned", { text, pinned }),
  },
  dictionary,
  loadDefaultPreferences: () => invoke<Preferences>("restored_default_preferences"),
  /* mobile host services are injected after host_capabilities resolves */
};
const panelClients: {
  keyboard: PanelClient;
  handwriting: PanelClient;
  voice: VoicePanelClient;
  cloudClipboard: CloudClipboardPanelClient;
  cloudDictionary: CloudDictionaryPanelClient;
  emoji: EmojiPanelClient;
} = {
  keyboard: {
    beginWindowDrag: () => getCurrentWindow().startDragging(),
    close: () => invoke("close_panel", { label: "keyboard-panel" }),
    openVoice: () => invoke("open_voice_panel"),
    rememberInputTarget: () => invoke("remember_input_target"),
    sendKey: (request) => invoke("send_key", { request }),
  },
  handwriting: {
    beginWindowDrag: () => getCurrentWindow().startDragging(),
    close: () => invoke("close_panel", { label: "handwriting-panel" }),
    rememberInputTarget: () => invoke("remember_input_target"),
    recognizeHandwriting: (request) => invoke("recognize_handwriting", { request }),
    submitHandwritingCandidate: (candidate) =>
      invoke("submit_handwriting_candidate", { candidate }),
    copyHandwritingCandidate: (text) => invoke("copy_text", { text }),
  },
  voice: {
    maxSubmitBytes: 4096,
    beginWindowDrag: () => getCurrentWindow().startDragging(),
    close: () => invoke("close_panel", { label: "voice-panel" }),
    rememberInputTarget: () => invoke("remember_input_target"),
    loadVoiceLanguage: () => invoke<string>("voice_input_language"),
    ...createVoiceRecognitionClient(invoke, (listener) =>
      listen<{
        request_id: string;
        text: string;
        final: boolean;
        phase?: "recording" | "recognizing" | "polishing";
        level?: number;
      }>("voice-update", (event) => listener(event.payload)),
    ),
    sendText: (text) => invoke("send_text", { text }),
    sendVoiceText: (text) => invoke("send_voice_text", { text }),
    copyText: (text) => invoke("copy_text", { text }),
  },
  cloudClipboard: {
    canSendText: () => invoke<boolean>("cloud_clipboard_can_send_text"),
    close: () => invoke("close_panel", { label: "cloud-clipboard-panel" }),
    rememberInputTarget: () => invoke("remember_input_target"),
    sendText: (text) => invoke("send_text", { text }),
    copyText: (text) => invoke("copy_text", { text }),
    request: (action: CloudClipboardAction) => invoke("cloud_clipboard_request", { action }),
  },
  cloudDictionary: {
    close: () => invoke("close_panel", { label: "cloud-dictionary-panel" }),
    request: (action: CloudDictionaryAction) => invoke("cloud_dictionary_request", { action }),
  },
  emoji: {
    close: () => invoke("close_panel", { label: "emoji-panel" }),
    rememberInputTarget: () => invoke("remember_input_target"),
    sendText: (text) => invoke("send_text", { text }),
    copyText: (text) => invoke("copy_text", { text }),
    loadCatalog: () =>
      invoke<{
        emoji: EmojiCatalogGroup[];
        kaomoji: EmojiCatalogGroup[];
        symbols: EmojiCatalogGroup[];
        unavailable?: ("emoji" | "kaomoji" | "symbols")[];
      }>("load_emoji_catalog"),
    clipboard: {
      list: () =>
        invoke<ClipboardHistoryEntry[]>("list_clipboard_history").then((entries) =>
          entries.map((entry) => entry.text),
        ),
      isEnabled: async () => (await client.load()).preferences.clipboard_history ?? false,
      enable: async () => {
        const snapshot = await client.load();
        if (!snapshot.preferences.clipboard_history) {
          await client.save(snapshot.revision, {
            ...snapshot.preferences,
            clipboard_history: true,
          });
        }
      },
      onChanged: async (listener) => {
        const stopHistory = await listen("clipboard-history-changed", () => listener());
        try {
          const stopPreferences = await listen("preferences-changed", () => listener());
          return () => {
            stopHistory();
            stopPreferences();
          };
        } catch (error) {
          stopHistory();
          throw error;
        }
      },
      remove: (text) => invoke("remove_clipboard_history", { text }),
      clear: () => invoke("clear_clipboard_history"),
      sync: () =>
        invoke<ClipboardHistoryEntry[]>("sync_clipboard_history").then((entries) =>
          entries.map((entry) => entry.text),
        ),
      copy: (text) => invoke("copy_text", { text }),
    },
  },
};
const panel = new URLSearchParams(window.location.search).get("panel");
// 桌面的手写面板在宿主列出手写模型时第一次打开就下载它，下载失败时就地设置下载镜像。客户端固定为模块级对象，避免每次渲染换一个 client 让面板重置识别队列。
const desktopHandwritingClient: PanelClient = {
  ...panelClients.handwriting,
  resourcePacks,
  modelMirror: savedModelMirror(client),
};
function DesktopHandwriting({ theme }: { theme: "dark" | "light" }) {
  const platform = useHostPlatform(client.host);
  return (
    <HandwritingPanel
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

function DesktopPanelTheme({
  preferences,
  surface,
  children,
}: {
  preferences: Pick<SettingsClient, "load" | "onPreferencesChanged">;
  surface: "handwriting" | "voice" | "emoji";
  children: (theme: "dark" | "light") => ReactNode;
}) {
  const snapshot = usePreferencesSnapshot(preferences);
  const surfaceTheme =
    surface === "handwriting"
      ? snapshot?.preferences.handwriting_theme
      : surface === "voice"
        ? snapshot?.preferences.voice_theme
        : snapshot?.preferences.emoji_theme;
  return children(useCandidatePreviewTheme(snapshot?.preferences.theme, surfaceTheme));
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
      const state = {
        ...(current && typeof current === "object" ? current : {}),
        msimeSettings: true,
        panel: next,
      };
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
  useEffect(() => {
    const onPopState = (event: PopStateEvent) => {
      const state = event.state;
      if (state?.msimeSettings === true && typeof state.panel === "string") {
        setMobilePanel(state.panel as NonNullable<typeof mobilePanel>);
      } else if (mobilePanelRef.current !== null) {
        setMobilePanel(null);
      }
    };
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
    window.addEventListener("popstate", onPopState);
    window.addEventListener("msime-mobile-panel", onNativePanel);
    window.addEventListener("msime-settings-page", onNativeSettingsPage);
    return () => {
      window.removeEventListener("popstate", onPopState);
      window.removeEventListener("msime-mobile-panel", onNativePanel);
      window.removeEventListener("msime-settings-page", onNativeSettingsPage);
    };
  }, []);
  // The host menu entry that started this window names a section; resolve it
  // before mounting so the page never opens on one and then jumps.
  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    if (isTauri()) {
      void listen<string>("settings-route", (event) => {
        // An entry with no section only brings the window forward; it keeps the page in view.
        if (active && event.payload) requestSettingsPage(event.payload);
      })
        .then((stop) => {
          if (active) unsubscribe = stop;
          else stop();
        })
        .catch(() => {});
    }
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
      if (!active) return;
      if (linuxSetupStatus && !linuxSetupStatus.prepared) setLinuxSetup(linuxSetupStatus);
      setMacosInstall(macosInstallPending);
      setBootstrapRequired((android || ios) && !ready);
      setInitialPage(page ?? undefined);
      const hosted: SettingsClient = host
        ? {
            ...client,
            host,
            dictionary: isMobileHost(host.platform) ? mobileDictionary : dictionary,
            // Windows and macOS resolve the offline gloss in their native
            // candidate controllers, so the setting is real on both hosts.
            candidateEnglishGloss:
              host.platform === "linux" ||
              host.platform === "android" ||
              host.platform === "windows" ||
              host.platform === "macos" ||
              host.platform === "ios",
            // A file manager is only reachable on the desktop hosts; iOS and Android get the same
            // page without the button rather than one that fails when pressed.
            ...(host.typing_statistics
              ? {
                  typingStatistics: isMobileHost(host.platform)
                    ? typingStatistics
                    : {
                        ...typingStatistics,
                        openDirectory: () => invoke<void>("open_typing_statistics_directory"),
                      },
                }
              : {}),
            ...(host.platform === "macos" ||
            host.platform === "linux" ||
            host.platform === "windows"
              ? { resourcePacks }
              : {}),
            // The macOS input method writes diagnostic.log under Application Support, which the Finder hides; the host reveals it rather than asking the user to navigate there.
            ...(host.platform === "macos"
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
                    status: () => invoke("account_status"),
                    providers: () => invoke("account_providers"),
                    requestCode: (provider: string, target: string) =>
                      invoke("account_request_code", { provider, target }),
                    login: (challengeId: string, code: string) =>
                      invoke("account_login", { challengeId, code }),
                    googleLogin: () => invoke("account_google_login"),
                    googleCancel: () => invoke("account_google_cancel"),
                    profile: () => invoke("account_profile"),
                    rename: (displayName: string) => invoke("account_rename", { displayName }),
                    avatar: () => invoke<string | null>("account_avatar"),
                    chooseAvatar: () => invoke<AccountProfile | null>("account_choose_avatar"),
                    removeAvatar: () => invoke<AccountProfile>("account_remove_avatar"),
                    logout: (all: boolean) => invoke("account_logout", { all }),
                    deleteAccount: () => invoke("account_delete"),
                    clearExpired: () => invoke("account_forget"),
                  } satisfies AccountClient,
                  communityCandidateSkins: createDesktopCandidateSkinCommunity(invoke),
                  communityPlugins: createDesktopPluginCommunity(invoke),
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
      unsubscribe?.();
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
        onComplete={() => setLinuxSetup(null)}
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
  const cloudDictionary = {
    ...panelClients.cloudDictionary,
    ...cloudDictionaryCapabilities(settingsClient.host?.platform),
    ...(isMobileHost(settingsClient.host?.platform)
      ? {
          downloadToLocal: (entry: CloudDictionaryEntry) =>
            downloadCloudEntryToLocal(entry, mobileDictionary),
        }
      : {}),
  };
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
  const capabilities = cloudDictionaryCapabilities(host?.platform);
  const cloudDictionary = {
    ...panelClients.cloudDictionary,
    ...capabilities,
    ...(isMobileHost(host?.platform)
      ? {
          downloadToLocal: (entry: CloudDictionaryEntry) =>
            downloadCloudEntryToLocal(entry, mobileDictionary),
        }
      : {}),
  };
  return <DesktopCloudDictionary client={cloudDictionary} />;
}
function DesktopEmojiPanel({
  theme,
  initialPage = "home",
  client: providedClient,
  close,
}: {
  theme: "dark" | "light";
  initialPage?: "home" | "clipboard";
  client?: EmojiPanelClient;
  close?: () => Promise<void>;
}) {
  const [emojiClient, setEmojiClient] = useState<EmojiPanelClient | null>(null);
  const panelLabel = initialPage === "clipboard" ? "clipboard-panel" : "emoji-panel";
  useEffect(() => {
    let active = true;
    const capability = isTauri()
      ? invoke<boolean>("supports_clipboard_paste").catch(() => false)
      : Promise.resolve(false);
    void capability.then((supported) => {
      if (!active) return;
      const baseClient = providedClient ?? panelClients.emoji;
      const closePanel = close ?? (() => invoke("close_panel", { label: panelLabel }));
      setEmojiClient(
        supported
          ? {
              ...baseClient,
              close: closePanel,
              clipboard: baseClient.clipboard
                ? {
                    ...baseClient.clipboard,
                    paste: (text) => invoke<void>("paste_clipboard_text", { text }),
                  }
                : undefined,
            }
          : { ...baseClient, close: closePanel },
      );
    });
    return () => {
      active = false;
    };
  }, [close, panelLabel, providedClient]);
  return emojiClient ? (
    <EmojiPanel client={emojiClient} theme={theme} initialPage={initialPage} />
  ) : (
    <StatusMessage role="status">正在连接面板…</StatusMessage>
  );
}

const content =
  panel === "keyboard" ? (
    <DesktopKeyboard client={panelClients.keyboard} preferences={client} />
  ) : panel === "handwriting" ? (
    <DesktopPanelTheme preferences={client} surface="handwriting">
      {(theme) => <DesktopHandwriting theme={theme} />}
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
      {(theme) => <DesktopEmojiPanel theme={theme} initialPage="clipboard" />}
    </DesktopPanelTheme>
  ) : panel === "emoji" ? (
    <DesktopPanelTheme preferences={client} surface="emoji">
      {(theme) => <DesktopEmojiPanel theme={theme} />}
    </DesktopPanelTheme>
  ) : panel === "vocabulary" ? (
    <VocabularyReviewPanel client={vocabularyReview} />
  ) : (
    <DesktopSettings />
  );
createRoot(document.getElementById("root")!).render(<StrictMode>{content}</StrictMode>);
