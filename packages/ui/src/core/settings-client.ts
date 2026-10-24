import type { NineKeyShuangpinLayoutReader } from "../settings/nine-key-shuangpin-profile";
import type { Preferences, Snapshot } from "./preferences-model";
import type { HostCapabilities } from "./host-capabilities";
import type { ProviderCredentialClient } from "./provider-credential-contracts";
import type { DictionaryCollectionsClient } from "../dictionary/dictionary-collections";
import type {
  BuiltinShuangpinProfile,
  ShuangpinCustomProblem,
  ShuangpinCustomProfile,
} from "../settings/shuangpin-custom-profile";
import type { AppThemeClient, FeedbackClient, HostChromeClient } from "./host-contracts";
import type { PickedApplication } from "../settings/app-input-mode-rules";
import type { VoiceDeviceReader } from "../voice/voice-device-model";
import type { LocalVoiceModelClient } from "../voice/local-models";
import type { ResourcePackClient } from "../settings/resource-packs";
import type { DictionaryClient, DictionaryManifest } from "../dictionary/dictionary-contracts";
import type { InputSourceStartupStatus } from "../settings/input-source-startup-model";
import type { NoticesClient } from "../settings/notice-banner";
import type { MacosInputModesClient } from "../settings/macos-input-modes";
import type { FontCatalogReader } from "../candidate/font-catalog";
import type { ResolveThemeRequest, ResolvedTheme } from "../theme/global-theme";
import type { AiSkinClient, CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import type { SkinCatalog } from "../skin/external-skins";
import type { CandidateSkinEditorClient } from "../skin/candidate-skin-draft";
import type { TypingStatisticsClient } from "../settings/typing-statistics-model";
import type { VocabularyReviewClient } from "../settings/vocabulary-review";
import type {
  McpClientId,
  McpFlag,
  McpInstallOutcome,
  McpServerStatus,
} from "../settings/mcp-connect";
import type { AccountClient, AppIconClient } from "../account/account-model";
import type { ChatClient } from "../chat/chat-model";
import type { HomePageActions } from "../keyboard/home-model";
import type { PluginClient } from "../settings/plugin-types";
import type { CustomHelpcodeSchema } from "../settings/helpcode-preferences";
import type { ClipboardHistoryClient } from "../settings/clipboard-history-model";
import type { CloudClipboardRequest } from "../settings/cloud-clipboard-send";
import type { MobileKeyboardFeedbackClient } from "../settings/mobile-keyboard-feedback";
import type { CommunitySkinClient } from "../community/community-skin-model";
import type { CommunityResourceClient } from "../community/community-resource-model";
import type { CandidateSkinCommunityClient } from "../community/community-candidate-skin-model";
import type { CommunityPluginClient } from "../community/community-plugin-model";
import type { SkinImageReader } from "../skin/skin-image";
import type { SkinFontReader } from "../skin/skin-font";
import type { UpdateCheckRequest, UpdateCheckResult } from "../settings/update-manifest";
import type { AiAssistantClient } from "./ai-assistant-client";
import type { ApiCredentialTestService, ApiCredentialTestResult } from "./api-credential-test";
import type { CloudDictionaryPanelClient } from "../keyboard/panel-client";

export interface SettingsClient {
  /** 宿主能力；未提供时按 user-agent 检测。 */
  host?: HostCapabilities;
  /** Android 适配平台的 Apple 风格键盘首页。 */
  home?: HomePageActions;
  /** 移动端账号命令只暴露用户与资料 DTO，不暴露会话令牌。 */
  account?: AccountClient;
  /** 移动宿主提供原生启动图标或备用图标选择。 */
  appIcon?: AppIconClient;
  /** 移动端账号命令提供已认证的 EveryAPI 聊天界面。 */
  chat?: ChatClient;
  /** Android 在原生宿主执行用户配置的 AI 服务请求。 */
  aiAssistant?: AiAssistantClient;
  /** Linux 把 AI 与翻译凭据写入在线服务仅所有者可访问的文件。 */
  providerCredentials?: ProviderCredentialClient;
  /** 原生宿主测试凭据，不向 webview 暴露服务商秘密。 */
  testApiCredential?: (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ) => Promise<ApiCredentialTestResult>;
  /** Android 社区命令提供有界的公开皮肤元数据与设计。 */
  communitySkins?: CommunitySkinClient;
  /** Android 社区命令提供词库与回复模板。 */
  communityResources?: CommunityResourceClient;
  /** 桌面社区命令发布、安装和评分候选窗口皮肤包。 */
  communityCandidateSkins?: CandidateSkinCommunityClient;
  /** 桌面社区命令发布、安装和评分插件包，安装到 `plugins` 管理的存储。 */
  communityPlugins?: CommunityPluginClient;
  listVoiceCaptureDevices?: VoiceDeviceReader;
  listFontFamilies?: FontCatalogReader;
  resolveFontFamilies?: (names: string[]) => Promise<string[]>;
  scanSkinCatalog?: () => Promise<SkinCatalog>;
  /** 「制作皮肤」的宿主一侧：按皮肤目录的规则读一份清单，或把编辑器做的皮肤经共享安装器装进本机皮肤目录。还要 `host.candidate_skin_editor` 为真，标签才出现。 */
  candidateSkinEditor?: CandidateSkinEditorClient;
  /** 宿主在已验证的资源目录中发现的自定义辅助码表。 */
  listHelpcodeSchemas?: () => Promise<CustomHelpcodeSchema[]>;
  /** 宿主用输入法相同的 `resolve`（`msime_client_resolve_theme`）解析主题颜色，并从自己的皮肤目录读取自定义主题包。桥接没有主题调用时不提供；选择器与内置预览读取 `themeCatalog`，不需要宿主。 */
  resolveTheme?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  /** 编辑中的自定义双拼表有什么问题，能用时是 `null`。与 MCP 保存前的校验结论相同，但宿主不为它留下任何东西，页面每改一个键都可以问。和 `builtinShuangpinTable` 都提供时，输入页才提供自定义双拼的编辑、导入和导出。 */
  validateShuangpinCustomProfile?: (
    profile: ShuangpinCustomProfile,
  ) => Promise<ShuangpinCustomProblem | null>;
  /** 内置双拼方案的键位表，编辑自定义方案时从它开始。 */
  builtinShuangpinTable?: (name: BuiltinShuangpinProfile) => Promise<ShuangpinCustomProfile | null>;
  /** 录键写入前的试写校验：把 `chord` 作为 `field` 的绑定问宿主，通过回规范写法；不通过给 `invalid`（写法不是合法组合键）或 `conflict`（与 `owner` 占用项相同）。与保存时的 `Preferences::validate` 走同一张占用表，不落盘；只有声明 `host.toggle_hotkey_chords` 的宿主提供。 */
  checkToggleHotkey?: (request: ToggleHotkeyCheckRequest) => Promise<ToggleHotkeyCheckResult>;
  /** 九键双拼的内置表与用户表校验，复用引擎的无状态布局接口。 */
  nineKeyShuangpinLayout?: NineKeyShuangpinLayoutReader;
  readSkinImage?: SkinImageReader;
  readSkinFont?: SkinFontReader;
  readSkinToolbarCss?: (id: string, relative?: string) => Promise<string | null>;
  openSkinDirectory?: () => Promise<void>;
  /** 在文件管理器中显示输入法诊断日志，便于复现后发送。宿主自行确定位置；没有可用文件管理器时不提供，页面也不显示按钮。 */
  openDiagnosticLogDirectory?: () => Promise<void>;
  /** 将导出文档写入用户下载目录并返回绝对路径；重名时可能带有「 (2)」后缀。使用保存选择器的宿主（HarmonyOS）在用户关闭选择器时返回 null，也可以拒绝并原样显示 Error 消息。webview 会丢弃下载链接的宿主（macOS WKWebView）提供此方法，否则页面退回下载链接。 */
  saveExport?: (name: string, contents: string) => Promise<string | null>;
  load(): Promise<Snapshot>;
  save(revision: number, preferences: Preferences): Promise<Snapshot>;
  onPreferencesChanged?(listener: (snapshot: Snapshot) => void): Promise<() => void>;
  dictionary?: DictionaryClient;
  /** 命名词库：新建、导入、启用停用、装社区词库。提供它的宿主在手机的「词库」页按词库列出，而不是按种类查词条。 */
  dictionaryCollections?: DictionaryCollectionsClient;
  /** 原子地恢复内置词库并清除全部学习数据；宿主只在真正能清除的平台（三个桌面宿主）上提供它。 */
  resetLearnedData?: () => Promise<void>;
  /** 读取恢复默认值将写入的偏好，不执行写入。宿主决定保留哪些凭据，以及仍指向同一服务的端点和模型；规则归偏好文档持有。 */
  loadDefaultPreferences?: () => Promise<Preferences>;
  /** 修复宿主读不到的偏好文档，与 Windows 修复无法解析的 config.toml 相同。宿主先在原位置旁备份损坏文件，再保留仍能识别的设置和服务密钥。文档已可读取且没有写入时，不提供 `backupPath`。 */
  recoverPreferences?: () => Promise<PreferencesRecovery>;
  /** 打开偏好文档所在目录，修复备份也保存在这里。 */
  openPreferencesDirectory?: () => Promise<void>;
  readAppVersion?: () => Promise<string>;
  /** 本次构建的发布日期 `YYYY-MM-DD`，关于页把它写在版本号后面。构建时拿不到日期（没有 git 的源码树、开发构建）时返回空字符串，页面只显示版本号。反馈和使用统计里的 `app_version` 仍只是版本号。 */
  readAppReleaseDate?: () => Promise<string>;
  /** 原生更新器自行检查、下载、验签并安装完整应用；提供时优先于仅查发布列表的入口。 */
  checkForApplicationUpdate?: () => Promise<void>;
  /** 经由 `msime_client_services::update_check`（Tauri 的 `update_check` 命令，或原生宿主上的 `msime_client_update_check`）把本平台、本版本最新的已发布版本与当前运行的版本号比较。未提供时关于页不提供检查更新。 */
  checkUpdate?: (request: UpdateCheckRequest) => Promise<UpdateCheckResult>;
  /** 宿主在后台已经找到的新版本（Linux 设置应用每天一次的后台检查），关于页打开时直接显示；没有时为 `null`。 */
  pendingUpdateCheck?: () => Promise<UpdateCheckResult | null>;
  openExternalUrl?: (url: string) => Promise<void>;
  /** 控制台应用通知；宿主读取并缓存通知，记住已关闭的条目。未提供时不显示。 */
  notices?: NoticesClient;
  /** macOS 打开随应用包发布的版本化第三方声明。 */
  openThirdPartyLicenses?: () => Promise<void>;
  /** 「输入时显示双拼键位提示」以前只存在 macOS 本机 defaults 里，现在是共享偏好 `shuangpin_keymap_hint`。文档里还没有这一项时，macOS 输入法仍按本机的旧选择显示键位图，设置页读这个值只为显示同一个状态；改动写进共享偏好。 */
  loadMacosShuangpinKeymap?: () => Promise<boolean>;
  copyText?: (text: string) => Promise<void>;
  /** 桌面宿主随设置应用提供 `msime-mcp`，返回其位置和 AI 助手调用它的入口。 */
  mcpServerStatus?: () => Promise<McpServerStatus>;
  /** 把条目（`args` 末尾加上 `flags`）写进助手的配置文件。已有条目只差权限参数时直接更新；其它不同的 `msime` 条目在未设 `replace` 时以 `mcp_entry_exists` 拒绝。 */
  installMcpClient?: (
    client: McpClientId,
    replace: boolean,
    flags: readonly McpFlag[],
  ) => Promise<McpInstallOutcome>;
  /** 移动宿主可以打开系统键盘或输入法设置。 */
  openSystemKeyboardSettings?: () => Promise<void>;
  /** 移动宿主将键盘声音和触觉反馈保存在原生偏好中。 */
  mobileKeyboardFeedback?: MobileKeyboardFeedbackClient;
  openScreenKeyboard?: () => Promise<void>;
  openHandwriting?: () => Promise<void>;
  openVoice?: () => Promise<void>;
  /** 打开背单词面板。没有面板窗口的宿主不提供，设置页保留内嵌复习，不显示无效的打开按钮。 */
  openVocabulary?: () => Promise<void>;
  openCloudClipboard?: () => Promise<void>;
  /** 设置窗口自己发云剪贴板请求，经登录的水杉账号或配置的自建中继；有它时本机剪贴板历史可以把一条发到云剪贴板。 */
  cloudClipboardRequest?: CloudClipboardRequest;
  /** 这个宿主的云剪贴板会按偏好 `cloud_clipboard_relay` 改走自建中继；有它时「剪贴板」页显示同步方式和中继的地址、令牌。 */
  cloudClipboardRelay?: boolean;
  openCloudDictionary?: () => Promise<void>;
  /** 桌面设置页内嵌的云词库客户端；提供时词库页显示「云词库」顶部标签。 */
  cloudDictionary?: CloudDictionaryPanelClient;
  restartInputMethod?: () => Promise<void>;
  /** macOS 先安装或更新独立的 InputMethodKit 包，再注册输入源。 */
  installInputSource?: () => Promise<void>;
  /** macOS 上哪几个输入模式已经加入输入法列表，用于「方案」里的「菜单栏入口」提示。 */
  macosInputModes?: MacosInputModesClient;
  /** macOS 每次启动安装或刷新输入法，此处报告结果。 */
  inputSourceStartup?: {
    /** 启动检查结束后返回结果；本次启动未检查时返回 `null`。每次调用都重新读取启用状态，不再次安装，可重复调用。 */
    status(): Promise<InputSourceStartupStatus | null>;
    /** 打开添加和启用输入源的系统设置页。 */
    openSettings(): Promise<void>;
  };
  /** macOS 用 Apple 设备端模型翻译离线词典未覆盖的中文候选（包括整句），仅支持系统设置中已下载的语言对。 */
  onDeviceTranslation?: {
    /** 输入法最近发现可下载但尚未下载的目标语言代码。 */
    downloadableLanguages(): Promise<string[]>;
    /** 打开可下载翻译语言的「语言与地区」。 */
    openSettings(): Promise<void>;
  };
  /** macOS 把已安装的输入源移入废纸篓，用户数据是否删除由参数明确指定。 */
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
  /** macOS 保留少量固定定位文件，状态根目录可移到其他卷。 */
  dataDirectory?: {
    status(): Promise<{ path: string; isDefault: boolean }>;
    pick(): Promise<string | null>;
    /** 数据已移动但宿主无法重启输入法时，`inputMethodRestarted` 为 false；缺失表示宿主不负责重启，或已成功重启。 */
    move(): Promise<{
      path: string;
      isDefault: boolean;
      retainedOldData: boolean;
      inputMethodRestarted?: boolean;
    }>;
  };
  /** 请求宿主选择已安装的本地语音模型目录（包含 msime-model.json），用户取消时返回 null。模型按路径加载，而文件输入只返回内容，因此必须由宿主提供。 */
  pickVoiceModelPath?: () => Promise<string | null>;
  /** 应用例外的「添加应用…」：宿主弹出系统的选应用对话框，交回规则用的标识（macOS 的 Bundle ID，Windows 的程序文件名）和应用名称，用户取消时为 null；所选不是应用程序时以 `not_application` 拒绝。没有它的宿主只能手填应用标识。 */
  pickApplication?: () => Promise<PickedApplication | null>;
  /** 应用例外列表的显示名称：按规则标识查本机装着的应用，查不到的不返回（macOS）。 */
  applicationNames?: (ids: string[]) => Promise<Record<string, string>>;
  /** 宿主持有的设备端语音模型库；提供它的宿主在模型管理器中提供 `local` 服务。 */
  localVoiceModels?: LocalVoiceModelClient;
  /** 按需下载的资源包（日文词库、「粤语、注音与笔画词库」、手写模型）。只有 macOS 提供：发布包不再内置它们，选用对应方案时由设置页下载。 */
  resourcePacks?: ResourcePackClient;
  windowControl?: (action: "minimize" | "maximize" | "restore" | "close") => Promise<void>;
  beginWindowDrag?: () => Promise<void>;
  resizeWindow?: (edge: "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw") => Promise<void>;
  onWindowStateChanged?: (
    listener: (maximized: boolean) => void,
    onError?: () => void,
  ) => Promise<() => void>;
  clipboard?: ClipboardHistoryClient;
  typingStatistics?: TypingStatisticsClient;
  /** 宿主未接入共享背单词入口时不提供。 */
  vocabularyReview?: VocabularyReviewClient;
  /** 宿主提供共享模糊拼音设置。 */
  fuzzyPinyin?: boolean;
  /** Android 提供与 Apple 兼容的触屏键盘方案显示与选择。 */
  touchKeyboardSchemes?: boolean;
  /** Android 提供与 Apple 当前实现一致的自定义触屏键盘设计器和渲染器。 */
  customTouchKeyboardSkins?: boolean;
  /** 命名的自定义设计存放在独立有界文件中，不进入热路径偏好。 */
  customSkinLibrary?: CustomSkinLibraryClient;
  /** Android 使用账号的 AI 皮肤抽卡与绘图任务。 */
  aiSkins?: AiSkinClient;
  /** 移动与桌面宿主可显示随包提供的离线英文释义，不改变候选标识。 */
  candidateEnglishGloss?: boolean;
  /** 宿主在每行释义后绘制读音，例如英文 IPA 或日文罗马音。 */
  candidatePronunciation?: boolean;
  /** 桌面宿主的插件包存储与 @ 名称列表，供插件页使用。 */
  plugins?: PluginClient;
  /** 供词库页说明已安装的词库版本。将资源准备到沙箱的宿主才知道清单的位置，页面不接收路径。宿主无法回答时不提供，页面不绘制该段，避免报告错误版本。 */
  dictionaryManifest?: () => Promise<DictionaryManifest>;
  /** HarmonyOS 设置所用的季节应用主题（水杉四季和四个固定季节）。其他平台没有这一项，保持平台自己的 token 颜色。 */
  appTheme?: AppThemeClient;
  /** 给系统状态栏和导航栏着色以匹配页面；只有掌管自己窗口系统栏的宿主（HarmonyOS 手机）才提供。 */
  chrome?: HostChromeClient;
  /** 向服务发送应用内反馈报告；只通过 GitHub issue 途径反馈的宿主没有这一项。 */
  feedback?: FeedbackClient;
}

/** 一次切换热键试写：`field` 是 `keybindings` 的三个切换热键字段名，`chord` 是录下的写法（空串表示解绑），`keybindings` 与 `voice_ctrl_f9` 是页面当前草稿。 */
export interface ToggleHotkeyCheckRequest {
  field: "toggle_english_candidates" | "toggle_full_width" | "toggle_punctuation";
  chord: string;
  keybindings: Record<string, unknown>;
  voice_ctrl_f9: boolean;
  /** 录键传物理 DOM code，由宿主按当前布局归一；手工规范键名默认不重译。 */
  physical_recording?: boolean;
}

export type ToggleHotkeyCheckResult =
  | { ok: true; chord: string }
  | { ok: false; kind: "invalid"; detail: string }
  | { ok: false; kind: "conflict"; owner: string; chord: string };

export interface PreferencesRecovery {
  snapshot: Snapshot;
  backupPath?: string | null;
  salvaged: boolean;
}
