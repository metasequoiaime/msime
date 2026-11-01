import { StatisticsAuthentication } from '../keyboard/settings/StatisticsAuthentication';
import { ScreenCaptureAuthentication } from '../keyboard/settings/ScreenCaptureAuthentication';
import { ScreenCaptureProtection } from '../keyboard/settings/ScreenCaptureProtection';
import {
  ScreenCaptureProtectionDecision,
  ScreenCaptureProtectionPolicy,
  ScreenCaptureProtectionState
} from '../keyboard/settings/ScreenCaptureProtectionPolicy';
import { NativeCleanup } from '../keyboard/NativeCleanup';
import web_webview from '@ohos.web.webview';
import common from '@ohos.app.ability.common';
import client, { DoubaoFrameResult } from 'libmsimeclient.so';
import { HarmonyReadAloud } from '../speech/HarmonyReadAloud';
import { ReadAloudPolicy } from '../speech/ReadAloudPolicy';
import pasteboard from '@ohos.pasteboard';
import Want from '@ohos.app.ability.Want';
import bundleManager from '@ohos.bundle.bundleManager';
import BuildProfile from 'BuildProfile';
import { UIContext } from '@ohos.arkui.UIContext';
import { InputMethodListDialog } from '@ohos.inputMethodList';
import deviceInfo from '@ohos.deviceInfo';
import display from '@ohos.display';
import http from '@ohos.net.http';
import { installNoRedirectGuard } from '../network/HarmonyHttpSecurity';
import webSocket from '@ohos.net.webSocket';
import util from '@ohos.util';
import fs from '@ohos.file.fs';
import fileUri from '@ohos.file.fileuri';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { KeyboardSession } from '../keyboard/KeyboardSession';
import { StagedResources } from '../keyboard/StagedResources';
import { KeyboardScheme } from '../keyboard/KeyboardScheme';
import { HarmonyAccountCloudBridge } from '../account/HarmonyAccountCloudBridge';
import { AccountSessionAdoption } from '../account/AccountSessionAdoption';
import { utf8Length, utf8WriteComplete } from '../keyboard/Utf8';
import { TextPolicy } from '../keyboard/TextPolicy';
import { TypingStatisticsPolicy } from '../keyboard/TypingStatisticsPolicy';
import { niutransAuth, tencentAuthorization } from '../keyboard/CredentialCrypto';
import { HarmonyVoiceCaptureDevices } from '../keyboard/input/HarmonyVoiceCaptureDevices';
import { KeyboardFeedbackBridge, MobileKeyboardFeedback } from '../keyboard/KeyboardFeedbackBridge';
import { KeySoundPreview } from '../keyboard/KeySoundPreview';
import inputMethod from '@ohos.inputMethod';
import { PreferenceRevisionPolicy } from '../keyboard/input/PreferenceRevisionPolicy';
import { PreferencesErrorCode } from '../keyboard/settings/PreferencesErrorCode';
import picker from '@ohos.file.picker';
import { SkinImportPolicy } from '../keyboard/skin/SkinImportPolicy';
import {
  PickedEntryKind, PluginFolderScan, PluginImportPolicy, PluginImportRefusal
} from '../keyboard/settings/PluginImportPolicy';
import EntryAbility from '../entryability/EntryAbility';
import { SettingsLinkRequest } from '../keyboard/SettingsPageLink';
import { ImeEnabledState, OnboardingState, OnboardingStatePolicy, SetupState }
  from '../keyboard/input/OnboardingStatePolicy';
import InputMethodSubtype from '@ohos.InputMethodSubtype';
import window from '@ohos.window';
import { AppThemeStore } from '../keyboard/skin/AppThemeStore';
import { FeedbackSettings } from '../keyboard/KeyboardFeedback';
import { VoiceCaptureDevice } from '../keyboard/input/VoiceCaptureDevicePolicy';
import { OsVersionPolicy } from '../keyboard/input/OsVersionPolicy';
import {
  AiCatalogPage, AiModelCatalogPolicy
} from '../keyboard/settings/AiModelCatalogPolicy';
import { SettingsFormFactorCapabilities, SettingsFormFactorProjection }
  from '../keyboard/settings/SettingsFormFactorCapabilities';
import { VocabularyAction, VocabularyReviewRequest } from '../keyboard/settings/VocabularyReviewRequest';
import { LocalVoiceModelAction, LocalVoiceModelPolicy } from '../keyboard/settings/LocalVoiceModelPolicy';
import { HTTP_ASR_CHAT_AUDIO, HttpAsrConfigurationPolicy } from '../keyboard/input/HttpAsrConfigurationPolicy';
import { AiAuthenticationPolicy } from '../keyboard/settings/AiAuthenticationPolicy';
import { CredentialStatusPolicy } from '../keyboard/settings/CredentialStatusPolicy';
import { AiEndpoint } from '../keyboard/settings/AiEndpoint';
import {
  CustomTranslationProbeDescriptor,
  CustomTranslationProbePolicy,
  CustomTranslationProbeRequest
} from '../keyboard/settings/CustomTranslationProbePolicy';
import { HarmonyTelemetry } from '../telemetry/Telemetry';
import { TelemetryPolicy } from '../telemetry/TelemetryPolicy';
import { NoticeItem, NoticePolicy } from '../notices/NoticePolicy';
import { HarmonySharedSandboxPolicy } from '../keyboard/HarmonySharedSandboxPolicy';
import { SharedStateMirror } from '../keyboard/SharedStateMirror';
import { SharedStateSync } from '../keyboard/SharedStateSync';
import { UserWordsMirror } from '../keyboard/UserWordsMirror';
import {
  FileIdentity, SETTINGS_MAX_CHANGES, SETTINGS_QUEUE_ROUNDS, SETTINGS_RETRY_MS, UserWordsMirrorPolicy, UserWordsReply
} from '../keyboard/UserWordsMirrorPolicy';
import { HarmonyPrivacyConsent } from '../privacy/HarmonyPrivacyConsent';
import { ConsentState, PrivacyConsentPolicy } from '../privacy/PrivacyConsentPolicy';
import { PrivacyConsentDialog } from '../privacy/PrivacyConsentDialog';
import { AiGenerationGate } from '../keyboard/AiGenerationGate';
import { CommunityContributionGate } from '../keyboard/CommunityContributionGate';

/** `{root}` for the list, `{root, id, mirror}` for an install, `{root, id, files}` for an import from local files, `{root, id}` for a removal, `{id}` for a cancel. */
interface LocalVoiceModelRequest {
  root?: string;
  id?: string;
  mirror?: string;
  files?: string[];
}

/** 「从文件导入」一次最多让用户选几个文件；目录里的模型最多要两个。 */
const MAX_VOICE_MODEL_IMPORT_PICKS: number = 8;

/** 页面发来的一次按键反馈操作：`load`、`save`、`preview`（试振动），或 `preview_sound`（试听 `pack`，音量 `volume`）。 */
interface KeyboardFeedbackAction {
  operation: string;
  settings?: MobileKeyboardFeedback;
  strength?: string;
  pack?: string;
  volume?: number;
}

interface KeyboardFeedbackReply {
  ok: boolean;
  value: MobileKeyboardFeedback;
  error: string;
}

interface ExportSaveRequest {
  name: string;
  contents: string;
}

/** 「关于」页随 `update_check` 发来的内容：当前运行的版本号，以及页面知道时的版本 id 和架构。 */
interface UpdateCheckPayload {
  current_version: string;
  edition?: string;
  arch?: string;
}

/** `msime_client_update_check` 的请求；平台始终是本宿主的。 */
interface UpdateCheckRequest {
  platform: string;
  current_version: string;
  edition?: string;
  arch?: string;
}

interface TextReply {
  ok: boolean;
  value: string;
  error: string;
}

interface RevisionValue {
  revision: number;
}

interface RevisionReply {
  ok: boolean;
  value?: RevisionValue;
}

/** What `repairPreferences` answers; only the repaired snapshot's revision is read here. */
interface RepairValue {
  snapshot: RevisionValue;
}

interface RepairReply {
  ok: boolean;
  value?: RepairValue;
}

interface OnboardingReply {
  ok: boolean;
  value: boolean;
  /** False until the answer has been computed; the page waits rather than acting on a default. */
  ready: boolean;
  error: string;
}

/** `setupStatus`：两项设置状态，以及是否已经读到。 */
interface SetupStatusReply {
  ok: boolean;
  ready: boolean;
  value: SetupState;
}

interface OnboardingSeenValue { seen: boolean; }

interface OobeStatusValue {
  resourcesReady: boolean | null;
  systemEnabled: boolean | null;
  systemCurrent: boolean | null;
  preferencesSaved: boolean | null;
  onboardingSeen: boolean;
}

interface OobeStatusReply {
  ok: boolean;
  value: OobeStatusValue;
}

/** 一次应用主题请求：`load`、带 id 的 `save`、带页面深色标志的 `resolve`，或 `catalog`。 */
interface AppThemeAction {
  operation: string;
  id?: string;
  dark?: boolean;
}

interface AppThemeIdValue {
  id: string;
}

interface AppThemeReply {
  ok: boolean;
  value?: AppThemeIdValue;
  error?: string;
}

/** `msime_client_resolve_app_theme` 读取的请求；`month` 是设备本地日历的月份。 */
interface AppThemeResolveRequest {
  app_theme: string;
  month: number;
  dark: boolean;
}

/** 页面给系统栏的颜色，以及页面是否为深色（深色时栏内容转为浅色）。 */
interface SystemBarsRequest {
  background: string;
  navigationBar: string;
  dark: boolean;
}

interface FlagReply {
  ok: boolean;
  value: boolean;
}

/** 页面经 `privacyConsent` 发来的动作：`status` 只读当前授权，`review` 另外打开授权弹窗。 */
interface PrivacyConsentAction {
  operation?: string;
}

interface PrivacyConsentReply {
  ok: boolean;
  value: ConsentState;
}

/** `value` 是页面 id，与只带页面的旧链接同一个形状；`target` 是那一页的子目标，没有时为 null。 */
interface PendingPageReply {
  ok: boolean;
  value: string | null;
  target: string | null;
}

/** 在这里补上日期的 `summary`，即 `msime_client_typing_statistics` 读取的形式。 */
interface TypingStatisticsSummaryAction {
  operation: string;
  day: string;
  user_words?: number;
}

interface TypingStatisticsSkinAction {
  operation: string;
  id: string;
}

interface TypingStatisticsSummaryRequest {
  directory: string;
  action: TypingStatisticsSummaryAction;
}

interface TypingStatisticsSkinRequest {
  directory: string;
  action: TypingStatisticsSkinAction;
}

/** The installed family names, or a refusal the page can report rather than an empty list. */
interface FontFamilyReply {
  ok: boolean;
  value: string[];
  error: string;
}

/** The envelope every bridge reply uses, so a refusal is distinguishable from an empty result. */
interface VoiceCaptureDeviceReply {
  ok: boolean;
  value: VoiceCaptureDevice[];
  error: string;
}

/**
 * The settings window, which is the shared settings UI rather than a second one written here.
 *
 * Every host renders the same React surface from packages/ui and differs only in what it declares it
 * can do: the page draws a control when the host says the control is connected to something, so a
 * platform never shows a switch that saves and does nothing. Writing a HarmonyOS-shaped imitation of
 * that page would have meant maintaining a second, worse copy of every screen in it.
 *
 * The bundle is built into rawfile and loaded from there, so the window needs no network. Everything
 * it needs from the system comes through one injected object, which is deliberately thin: JSON in,
 * JSON out, the same shape the shared C ABI already uses, leaving the bridge with no opinion about
 * preferences and nothing to keep in sync with the schema.
 */

const BRIDGE_NAME: string = 'msimeHarmony';
const PLATFORM: string = 'harmony';
/** 欢迎开屏是否已播放过的标记，与应用主题文件同在状态目录。 */
const SPLASH_SEEN_FILE: string = 'welcome-splash.json';
const ONBOARDING_SEEN_FILE: string = 'onboarding-seen.json';
// 页面一直没报告第一帧上屏时，启动画面最多盖这么久。
const LAUNCH_COVER_LIMIT_MS: number = 4000;
/** The settings page and native save bridge hold one complete export in memory. */
const MAX_DICTIONARY_EXPORT_BYTES: number = 32 * 1024 * 1024;

/** `{directory, platform, channel}` for `client.notices`; `{directory, id}` for `client.noticeDismiss`. */
interface NoticesRequest {
  directory: string;
  platform?: string;
  channel?: string;
  id?: string;
}

/** 页面的 AI 皮肤请求里这里只读 request_id：进度要带着它送回页面。 */
interface AiSkinAction {
  request_id?: string;
}

/**
 * The nested action documents for the two native libraries.
 *
 * Declared rather than written inline because ArkTS requires every object literal to name a type.
 * They are the request shapes `msime_client_community_resource_library` and
 * `msime_client_keyboard_skin_trial` parse, so the names say which side owns them.
 */
interface ResourceLibraryRemove {
  operation: string;
  id: string;
}

interface ResourceLibraryRequest {
  file: string;
  action: ResourceLibraryRemove;
}

interface SkinTrialFinish {
  operation: string;
  id: string;
  keep: boolean;
}

interface SkinTrialRequest {
  directory: string;
  action: SkinTrialFinish;
}

/** One community resource request; only the two local library operations are handled here. */
interface CommunityResourceAction {
  operation: string;
  id?: string;
  item?: Object;
}

/** 社区皮肤请求里这里只处理不联网的 finish_trial，以及装好皮肤以后刷新页面的偏好。 */
interface CommunitySkinAction {
  operation: string;
  id: string;
  keep?: boolean;
}

interface CloudDownloadEntry {
  kind: string;
  code: string;
  word: string;
  weight: number;
}

interface LocalDictionaryEntry {
  kind: string;
  key: string;
  value: string;
  weight: number;
}

interface LocalDictionaryEditRequest {
  operation: string;
  previous: LocalDictionaryEntry | null;
  replacement: LocalDictionaryEntry;
  request_id: string;
}

interface TypingStatisticsAction {
  operation: string;
  enabled?: boolean;
  retention?: string;
  day?: string;
  /** `summary`：用户已添加的词数，供词数徽章使用；页面不知道时省略。 */
  user_words?: number;
  /** `record_skin`：刚应用的皮肤，供「换装达人」徽章使用。 */
  id?: string;
}

/** 页面经 `screen_capture_protection_settings` 发来的动作：`load` 读当前值，`save` 按切换规则保存 `enabled`。 */
interface ScreenCaptureProtectionAction {
  operation?: string;
  enabled?: boolean;
}

/** One 插件 page request; only the operation is read here, the rest travels to `msime_client_plugins` as the page wrote it. */
interface PluginPageAction {
  operation: string;
}

/** The page's import request: which picker to show. */
interface PluginImportRequest {
  source: string;
}

/** 「制作皮肤」页面发来的请求：`check` 或 `create`，草稿是文件夹名、`skin.toml` 全文和 base64 图片。页面不给路径。 */
interface CandidateSkinEditorPayload {
  operation?: string;
  package_id?: string;
  manifest?: string;
  files?: Record<string, string>;
  replace?: boolean;
}

/** `msime_client_candidate_skin_editor` 的请求；`directory` 由宿主填成自己的皮肤根目录，只有 `create` 带它。 */
interface CandidateSkinEditorRequest {
  operation: string;
  package_id: string;
  manifest: string;
  files: Record<string, string>;
  replace: boolean;
  directory?: string;
}

/** `msime_client_plugins`'s import action, naming the staged copy of what was picked. */
interface PluginImportAction {
  operation: string;
  source: string;
}

interface PluginImportCall {
  state_root: string;
  sound_packs: string;
  action: PluginImportAction;
}

/** 单表导入复用 Rust 插件边界，页面不能指定状态目录。 */
interface HelpcodeImportAction {
  operation: string;
  file_name: string;
  text: string;
}

interface HelpcodeImportCall {
  state_root: string;
  action: HelpcodeImportAction;
}

interface TypingStatisticsDocument {
  enabled: boolean;
  total: number;
  days: Record<string, number>;
  detail: Object;
  dailyDetails: Record<string, Object>;
}

interface TypingStatisticsReply {
  ok: boolean;
  value: TypingStatisticsDocument;
  error: string;
}

interface TypingStatisticsStatusValue {
  statistics: TypingStatisticsDocument;
  availability: string;
  lastWrittenMs: number | null;
}

interface TypingStatisticsStatusReply {
  ok: boolean;
  value: TypingStatisticsStatusValue;
}

interface AiModelsRequest {
  endpoint: string;
  token: string;
  provider: string;
}

interface AiTestRequest {
  endpoint: string;
  model: string;
  prompt: string;
  token: string;
  text: string;
}

interface ApiCredentialRequest {
  service: string;
  config: ApiCredentialConfig;
}

interface ApiCredentialConfig {
  endpoint?: string;
  model?: string;
  token?: string;
  provider?: string;
  api_key?: string;
  secret_id?: string;
  secret_key?: string;
  region?: string;
  app_id?: string;
  apikey?: string;
  auth_mode?: string;
  app_key?: string;
  resource_id?: string;
  doubao_enable_itn?: boolean;
  doubao_enable_punc?: boolean;
  doubao_enable_ddc?: boolean;
  doubao_boosting_table_id?: string;
}

interface AiMessage {
  content?: string;
}

interface AiChoice {
  message?: AiMessage;
}

interface AiCompletionDocument {
  choices?: AiChoice[];
}

interface AiMessageRequest {
  role: string;
  content: string;
}

interface AiTestBody {
  model: string;
  stream: boolean;
  temperature: number;
  messages: AiMessageRequest[];
}

interface AiReply<T> {
  ok: boolean;
  value: T;
  error: string;
}

interface ApiCredentialResult {
  ok: boolean;
  message: string;
}

interface ApiCredentialReply {
  ok: boolean;
  value: ApiCredentialResult;
}

interface CustomTranslationProbeDescriptorReply {
  ok: boolean;
  value?: CustomTranslationProbeDescriptor | null;
}

interface CustomTranslationProbeGlossReply {
  ok: boolean;
  value?: string | null;
}

interface ChatCredentialBody {
  model: string;
  stream: boolean;
  max_tokens: number;
  messages: AiMessageRequest[];
  thinking?: ThinkingOptions;
  enable_thinking?: boolean;
}

interface TencentTranslationBody {
  Source: string;
  Target: string;
  ProjectId: number;
  SourceTextList: string[];
}

interface TencentTranslationDocument {
  Response?: TencentTranslationResponse;
}

interface TencentTranslationResponse {
  TargetTextList?: string[];
  Error?: Object;
}

interface NiuTransDocument {
  tgtText?: string;
  errorCode?: string | number;
  errorMsg?: string;
}

interface ThinkingOptions {
  type: string;
}

interface AiModelHeaders {
  Accept: string;
  Authorization?: string;
  'x-api-key'?: string;
  'anthropic-version'?: string;
}

interface DoubaoResponseDocument {
  error?: Object | null;
  code?: number;
  payload_msg?: DoubaoResponseDocument;
}

interface DoubaoStartRequestBody {
  model_name: string;
  enable_itn: boolean;
  enable_punc: boolean;
  enable_ddc: boolean;
  show_utterances: boolean;
  result_type: string;
  corpus?: DoubaoCorpus;
}

interface DoubaoCorpus {
  boosting_table_id: string;
}

interface DoubaoUser {
  uid: string;
}

interface DoubaoAudio {
  format: string;
  codec: string;
  rate: number;
  bits: number;
  channel: number;
}

interface DoubaoStartRequest {
  user: DoubaoUser;
  audio: DoubaoAudio;
  request: DoubaoStartRequestBody;
}

@Entry
@Component
struct Settings {
  private controller: web_webview.WebviewController = new web_webview.WebviewController();
  private context: common.UIAbilityContext = this.getUIContext().getHostContext() as common.UIAbilityContext;
  private bridge: SettingsBridge = new SettingsBridge();
  private inputMethodDialog: CustomDialogController = new CustomDialogController({
    builder: InputMethodListDialog(),
    onWillDismiss: (action: DismissDialogAction): void => {
      action.dismiss();
      this.bridge.refreshSetupState();
    }
  });
  // 隐私政策的授权弹窗。`privacyGranted` 是当前版本是否已同意，从「隐私」页重新打开时弹窗据此多显示一句撤回的说明。
  @State privacyGranted: boolean = false;
  // 这一次弹窗是否必须做出选择：首次启动和隐私政策更新后的弹窗不能用返回手势关掉，从「隐私」页重新打开的可以。
  private privacyRequired: boolean = false;
  // 本进程是否已经自动弹过一次；回到前台时的 onPageShow 不再弹。
  private privacyPrompted: boolean = false;
  private privacyDialog: CustomDialogController = new CustomDialogController({
    builder: PrivacyConsentDialog({
      granted: $privacyGranted,
      onDecide: (granted: boolean): void => this.decidePrivacy(granted),
      onOpenDocument: (url: string): void => this.bridge.openExternalUrl(url)
    }),
    autoCancel: false,
    alignment: DialogAlignment.Center,
    onWillDismiss: (action: DismissDialogAction): void => {
      if (!this.privacyRequired) action.dismiss();
    }
  });
  // Notices from the MSIME service that the user has not dismissed, newest first; the card shows the first.
  @State notices: NoticeItem[] = [];
  private noticeController: web_webview.WebviewController = new web_webview.WebviewController();
  private noticeAttached: boolean = false;
  // The notice whose body the card's web view holds, so a refresh that changes nothing does not reload it.
  private noticeShown: string = '';
  // 让通知卡片不出现在欢迎流程上：读到设置状态之前一直压住，读到之后只要流程还在显示就继续压住。设置步骤上方的卡片会把步骤往下挤，与该屏唯一要用户做的事争夺注意力。
  @State noticeHeld: boolean = true;
  // 页面上次要求状态栏采用的颜色，即页面自己的背景（应用主题下为当季背景）；页面提出前为空，此时由资源默认值代替。通知卡片背后的列也用这个颜色，让卡片边距露出的是页面，而不是另一种灰色的色带。
  @State pageBackground: string = '';
  // 页面第一帧上屏之前盖在网页上的启动画面，见 `launchCoverView`。
  @State launchCover: boolean = true;
  /** Bumped for every notice fetch and when the page disappears, so late answers cannot repaint it. */
  private noticeRequest: number = 0;

  aboutToAppear(): void {
    this.bridge.attach(this.context);
    this.bridge.uiContext = this.getUIContext();
    this.bridge.onInputMethodPicker = (): void => this.inputMethodDialog.open();
    this.bridge.onPrivacyReview = (): void => this.openPrivacyDialog(false);
    this.bridge.useWebview(this.controller);
    this.bridge.onNoticeHold = (held: boolean): void => {
      this.noticeHeld = held;
    };
    this.bridge.onPageBackground = (colour: string): void => {
      this.pageBackground = colour;
    };
    this.bridge.onLaunchPainted = (): void => {
      this.launchCover = false;
    };
    // 页面没能报告（脚本出错、桥接没连上）时也不能一直盖着：几秒后撤掉，露出页面自己的错误说明。走桥接的同一个入口，盖着期间存下的系统栏颜色也一并补上。
    setTimeout((): void => {
      this.bridge.launchPainted();
    }, LAUNCH_COVER_LIMIT_MS);
    // Started here rather than on demand: the page reads the answer synchronously, and the reply
    // carries whether it is ready so the page waits instead of acting on a default.
    this.bridge.refreshSetupState();
    this.bridge.watchSetup();
    this.loadNotices();
    EntryAbility.onActivated = (): void => {
      this.bridge.resumeStatistics();
      this.bridge.syncShared();
      this.bridge.refreshPreferencesIfChanged();
      this.loadNotices();
      // 从系统的输入法界面返回就是设置状态改变的途径，所以在这里重新读取，与 Android 的 `ImeSetup` 在获得焦点时重新检查一样。
      this.bridge.refreshSetupState();
      // 每次返回都重新解析而不是靠定时器：水杉四季跟随月份，跨月一直开着的窗口不应保留上个月的颜色。
      this.bridge.notifyAppThemeChanged();
    };
    EntryAbility.onDeactivated = (): void => {
      this.bridge.lockStatistics();
      this.bridge.publishShared();
    };
    EntryAbility.onOpenPage = (link: SettingsLinkRequest): void => {
      this.bridge.openPage(link);
    };
  }

  /**
   * 首次启动（或隐私政策更新后第一次打开）时弹出授权弹窗。放在页面显示时而不是 aboutToAppear：页面还没显示时弹窗打不开。弹窗盖在启动画面和网页上，网页照常加载，但在用户同意之前桥接拒绝所有联网请求（`PrivacyConsentPolicy`）。
   */
  onPageShow(): void {
    if (this.privacyPrompted || !HarmonyPrivacyConsent.shouldPrompt()) {
      return;
    }
    this.privacyPrompted = true;
    this.openPrivacyDialog(true);
  }

  private openPrivacyDialog(required: boolean): void {
    this.privacyRequired = required;
    this.privacyGranted = HarmonyPrivacyConsent.networkAllowed();
    this.privacyDialog.open();
  }

  /** 用户在弹窗里选了「同意」或「不同意」：记下来、告诉键盘和页面；同意后补上公告，拒绝后撤掉公告卡片。 */
  private decidePrivacy(granted: boolean): void {
    this.privacyRequired = false;
    this.privacyGranted = granted;
    this.bridge.decidePrivacy(granted);
    this.loadNotices();
  }

  /**
   * 系统返回手势和返回键先走页面自己的历史。
   *
   * 页面为每个子页面和引导步骤压入一条历史记录并监听 `popstate`，所以在 web view 里后退就是回到上一屏。只有无处可退时手势才交给系统，即退出应用。
   */
  onBackPress(): boolean {
    try {
      if (this.controller.accessBackward()) {
        this.controller.backward();
        return true;
      }
    } catch (error) {
      // 控制器挂到 web view 之前会抛异常；这一刻的返回手势交给系统。
      KeyboardLog.error(`settings history unavailable: ${describeError(error)}`);
    }
    return false;
  }

  /**
   * 在窗口打开或回到前台时获取公告，从不按定时器获取：client-services 在一次获取后的一分钟内都用自己的副本回答，这正是服务希望被询问的频率。
   */
  private loadNotices(): void {
    const generation: number = ++this.noticeRequest;
    // 拉取公告要联网，没有隐私政策的授权就不拉，已显示的公告卡片也撤掉。
    if (!HarmonyPrivacyConsent.networkAllowed()) {
      this.notices = [];
      this.renderNotice();
      return;
    }
    const request: NoticesRequest = {
      directory: `${this.context.filesDir}/state/notices`, platform: PLATFORM, channel: 'app'
    };
    client.notices(JSON.stringify(request)).then((reply: string): void => {
      if (!NoticePolicy.requestCurrent(generation, this.noticeRequest)) return;
      this.notices = NoticePolicy.items(reply);
      this.renderNotice();
    }).catch((error: Error): void => {
      KeyboardLog.error(`notices unavailable: ${describeError(error)}`);
    });
  }

  private dismissNotice(id: string): void {
    const request: NoticesRequest = { directory: `${this.context.filesDir}/state/notices`, id: id };
    const reply: string = client.noticeDismiss(JSON.stringify(request));
    if (!NoticePolicy.accepted(reply)) {
      KeyboardLog.error(`notice dismissal not saved: ${reply}`);
    }
    this.notices = NoticePolicy.without(this.notices, id);
    this.renderNotice();
  }

  /** Loads the first notice's body into the card, as base64 so no character in it is read as part of a data URL. */
  private renderNotice(): void {
    if (this.notices.length === 0) {
      // The card and its web view leave the page; a later notice attaches a new one.
      this.noticeAttached = false;
      this.noticeShown = '';
      return;
    }
    if (!this.noticeAttached || this.notices[0].id === this.noticeShown) {
      return;
    }
    const notice: NoticeItem = this.notices[0];
    try {
      const bytes: Uint8Array = util.TextEncoder.create('utf-8').encodeInto(NoticePolicy.page(notice.html));
      this.noticeController.loadData(new util.Base64Helper().encodeToStringSync(bytes), 'text/html', 'base64');
      this.noticeShown = notice.id;
    } catch (error) {
      KeyboardLog.error(`notice not shown: ${describeError(error)}`);
    }
  }

  /** A link in a notice opens in the system browser or mail application, never inside the card. */
  private openNoticeLink(url: string): void {
    const link: string | null = NoticePolicy.externalLink(url);
    if (link === null) {
      KeyboardLog.error('refused to open a notice link');
      return;
    }
    const want: Want = { action: 'ohos.want.action.viewData', entities: ['entity.system.browsable'], uri: link };
    this.context.startAbility(want).catch((error: Object): void => {
      KeyboardLog.error(`notice link would not open: ${describeError(error)}`);
    });
  }

  @Builder
  noticeCard() {
    Column() {
      Row() {
        Text(this.notices[0].title)
          .fontSize(16)
          .fontWeight(FontWeight.Medium)
          .fontColor($r('sys.color.ohos_id_color_text_primary'))
          .layoutWeight(1)
          .maxLines(2)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
        Button() {
          Text('✕').fontSize(14).fontColor($r('sys.color.ohos_id_color_text_secondary'))
        }
        .type(ButtonType.Circle)
        .width(32)
        .height(32)
        .backgroundColor(Color.Transparent)
        .accessibilityText('关闭公告')
        .onClick((): void => this.dismissNotice(this.notices[0].id))
      }
      .width('100%')
      Web({ src: 'about:blank', controller: this.noticeController })
        .javaScriptAccess(false)
        .domStorageAccess(false)
        .fileAccess(false)
        .onlineImageAccess(false)
        .backgroundColor(Color.Transparent)
        .onControllerAttached((): void => {
          this.noticeAttached = true;
          this.noticeShown = '';
          this.renderNotice();
        })
        .onLoadIntercept((event: OnLoadInterceptEvent): boolean => {
          const url: string = event.data.getRequestUrl();
          if (NoticePolicy.ownPage(url)) {
            return false;
          }
          this.openNoticeLink(url);
          return true;
        })
        .width('100%')
        .height(96)
    }
    .padding({ left: 16, right: 8, top: 8, bottom: 8 })
    .margin({ left: 16, right: 16, top: 8, bottom: 4 })
    .borderRadius(20)
    .backgroundColor($r('sys.color.ohos_id_color_card_bg'))
  }

  aboutToDisappear(): void {
    this.noticeRequest++;
    // 背单词卡片的喇叭用的引擎：没建过时什么也不做。
    HarmonyReadAloud.shutdown();
    // The ability outlives this page, so a handler left behind would hold a bridge whose webview
    // has gone.
    EntryAbility.onActivated = undefined;
    EntryAbility.onOpenPage = undefined;
    this.bridge.lockStatistics();
    EntryAbility.onDeactivated = undefined;
    this.bridge.unwatchSetup();
  }

  /**
   * 冷启动时盖在网页上的启动画面，和系统启动窗画的是同一个画面：启动窗底色上的水杉标志（`start_window_icon`），外面不套圆盘。底色取资源 `start_window_background`，浅色和深色各一份，跟随系统外观；标志两种外观相同。页面报告设置已经画出来（`launchPainted`）后撤掉它，所以从点图标到设置页是同一个画面淡出，中间不露出还没画出东西的网页，也不露出「正在读取设置」。
   *
   * 它和首次启动的网页开屏一样按网页区域居中，尺寸照开屏换算：标志是 150vp 见方的 `launch_logo.svg`。
   */
  @Builder
  launchCoverView() {
    Stack() {
      // 同步解码：异步加载时盖板第一帧没有标志，从启动窗接过来会闪一下空白。
      Image($r('app.media.launch_logo')).width(150).height(150).syncLoad(true)
    }
    .width('100%')
    .height('100%')
    .backgroundColor($r('app.color.start_window_background'))
    .transition(TransitionEffect.OPACITY.animation({ duration: 120 }))
  }

  build() {
    Column() {
      if (this.notices.length > 0 && !this.noticeHeld) {
        this.noticeCard()
      }
      Stack() {
        Web({ src: $rawfile('settings/index.html'), controller: this.controller })
          .javaScriptAccess(true)
          .domStorageAccess(true)
          .fileAccess(true)
          // 页面按 `prefers-color-scheme` 自己绘制深色主题。`WebDarkMode.Auto` 把系统的模式传给这个媒体查询；强制深色保持关闭，因为反转一个本就有深色配色的页面会把它反转回浅色。
          .darkMode(WebDarkMode.Auto)
          .forceDarkAccess(false)
          // Web 组件自己的底色和背后的页面一样：页面画出第一帧之前不再先闪一下 WebView 默认的白色（冷启动时它夹在深色启动窗和开屏之间）。
          .backgroundColor(this.pageBackground.length > 0 ? this.pageBackground : $r('app.color.settings_page_background'))
          .onPageEnd((event: OnPageEndEvent): void => {
            this.bridge.pageLoaded();
          })
          // Registered through the controller rather than the attribute: the attribute form takes an
          // async list but never settles the promises it hands the page, which is worse than the bug
          // it was meant to fix — the page hangs on its startup screen instead of quietly receiving
          // nothing. onControllerAttached runs before the first load, so the page still finds the
          // object already there rather than racing it.
          .onControllerAttached(() => {
            try {
              this.controller.registerJavaScriptProxy(this.bridge, BRIDGE_NAME,
                ['loadPreferences', 'savePreferences', 'hostCapabilities', 'appVersion', 'appReleaseDate',
                  'scanSkinCatalog', 'readSkinImage', 'readSkinFont', 'readSkinToolbarCss',
                  'dictionaryManifest', 'dictionary', 'nineKeyShuangpinLayout', 'accountName', 'cloudDictionaryDownload',
                  'typingStatistics', 'openExternalUrl', 'copyText',
                  'openSystemKeyboardSettings', 'listVoiceCaptureDevices', 'listFontFamilies',
                  'keyboardFeedback', 'onboardingStatus',
                  'showInputMethodPicker', 'importSkinFolder', 'startRequest', 'plugins',
                  'setupStatus', 'refreshSetupStatus', 'oobeStatus', 'retryOobeResources', 'appTheme', 'setSystemBars', 'splashSeen', 'markSplashSeen',
                  'onboardingStarted', 'onboardingFinished', 'pendingPage', 'launchPainted', 'privacyConsent'],
                []);
            } catch (error) {
              KeyboardLog.error(`settings bridge not registered: ${describeError(error)}`);
            }
          })
          .width('100%')
          .height('100%')
        if (this.launchCover) {
          this.launchCoverView()
        }
      }
      .width('100%')
      .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
    // web view 背后的页面自身背景，让页面绘制前的那段时间和它周围的空隙在深色模式下不会闪白；页面告知它绘制的颜色后就用那个颜色。
    .backgroundColor(this.pageBackground.length > 0 ? this.pageBackground : $r('app.color.settings_page_background'))
  }
}

/**
 * What the settings page is allowed to ask of the system.
 *
 * Each method answers the JSON the shared C ABI already returns, so a reply that the Engine refused
 * arrives at the page as the refusal rather than as an exception thrown across the bridge.
 */
class SettingsBridge {
  uiContext: UIContext | undefined = undefined;
  onInputMethodPicker: (() => void) | undefined = undefined;
  // 由页面组件设置：重新打开隐私政策的授权弹窗。
  onPrivacyReview: (() => void) | undefined = undefined;
  private stateDirectory: string = '';
  private sharedSandboxAvailable: boolean = false;
  // 与键盘交换状态的共享沙箱镜像，拿不到共享目录时为 null；`sharedPublishTimer` 合并短时间内的多次改动，只发布一次。
  private sharedMirror: SharedStateMirror | null = null;
  private sharedPublishTimer: number = -1;
  // 与键盘交换「我的词条」（`UserWordsMirrorPolicy`）：上次做完时本方日志和键盘发布的标记、是否正在做、正在做时又被叫了一次，以及没做完时的重试。
  private userWordsMark: string | null = null;
  private userWordsRunning: boolean = false;
  private userWordsAgain: boolean = false;
  private userWordsRetryTimer: number = -1;
  private dictionaryUpdateTimer: number = -1;
  // What the page was last handed, so an activation can tell a changed document from an unchanged
  // one. -1 until the page has loaded once, which never compares equal to a real revision.
  private observedRevision: number = -1;
  private onboardingRequired: boolean = false;
  private onboardingReady: boolean = false;
  // 页面告知欢迎流程结束后置位，之后的设置状态读取不再压住通知卡片。
  private onboardingDone: boolean = false;
  private resourcesReady: boolean | null = null;
  // 上次读到的两项设置状态；首次读取完成前都为 null。
  private setup: SetupState = { enabled: null, current: null } as SetupState;
  // 设置状态读取的计数，让先开始却后返回的读取不能覆盖更新的结果。
  private setupReads: number = 0;
  // 上次写进日志的设置状态原因。
  private setupDescription: string = '';
  // 设置文档是否已加载完成，让宿主到页面的调用只发给能接收的页面。在此之前页面会在启动时自己读取所需的一切。
  private pageReady: boolean = false;
  private statisticsAuthentication: StatisticsAuthentication = new StatisticsAuthentication();
  private screenCaptureAuthentication: ScreenCaptureAuthentication = new ScreenCaptureAuthentication();
  // 「防止截屏」正在认证或写入时为真，第二次切换直接拒绝，不叠两个认证框。
  private screenCaptureBusy: boolean = false;

  resumeStatistics(): void {
    this.webview?.runJavaScript("window.dispatchEvent(new Event('msime-statistics-active'))").catch((): void => {});
  }

  lockStatistics(): void {
    this.statisticsAuthentication.cancel();
    this.webview?.runJavaScript("window.dispatchEvent(new Event('msime-statistics-locked'))").catch((): void => {});
  }

  // 结构体的通知卡片开关：告知欢迎流程显示期间卡片是否应保持不出现在屏幕上。
  onNoticeHold: ((held: boolean) => void) | undefined = undefined;
  // 由页面组件设置：告知网页绘制的背景，即页面随系统栏颜色最后一次发来的值。
  onPageBackground: ((colour: string) => void) | undefined = undefined;
  // 由页面组件设置：页面第一帧已上屏，撤掉启动画面。
  onLaunchPainted: (() => void) | undefined = undefined;
  // 启动画面还盖着网页。这期间页面发来的系统栏颜色先存着，撤掉时再上：页面一挂载就给系统栏换成自己的颜色，盖板却要两帧后才撤，状态栏会先变色，和下面的启动画面错开一条色带。
  private launchCovered: boolean = true;
  private heldSystemBars: string | undefined = undefined;
  // 系统当前键盘改变时重新读取设置状态；在系统键盘对话框里选择键盘就会这样改变，而窗口始终没有离开前台。
  private imeChanged: (property: inputMethod.InputMethodProperty, subtype: InputMethodSubtype) => void =
    (property: inputMethod.InputMethodProperty, subtype: InputMethodSubtype): void => {
      this.refreshSetupState();
    };
  // Needed to hand the page a document: the bridge is called from the page, but this one call goes
  // the other way.
  private webview: web_webview.WebviewController | undefined = undefined;
  private context: common.UIAbilityContext | undefined = undefined;
  private accountBridge: HarmonyAccountCloudBridge | undefined = undefined;
  private cloudDownloadRequest: number = 0;
  // 「从文件导入」从打开选择器到安装结束的模型 id。复制进缓存的那段时间原生安装还没登记，原生取消找不到它，所以取消先记在 voiceImportsCancelled 里，由导入流程在复制每个文件前和开始安装前检查。
  private voiceImportsPending: Set<string> = new Set<string>();
  private voiceImportsCancelled: Set<string> = new Set<string>();
  // 「音效」里选了包之后的那一声试听。
  private keySoundPreview: KeySoundPreview = new KeySoundPreview();

  useWebview(controller: web_webview.WebviewController): void {
    this.webview = controller;
  }

  attach(context: common.UIAbilityContext): void {
    this.context = context;
    this.stateDirectory = `${context.filesDir}/state`;
    // AI 生成类功能的总闸取自 Rust 的宿主能力（`HostCapabilities.ai_generation`），鸿蒙版拿到资质之前为关；`startRequest` 据此拒绝 AI 对话、AI 皮肤和探 AI 服务的请求。
    AiGenerationGate.adopt(client.hostCapabilities(PLATFORM));
    // 社区投稿类功能的总闸取自 Rust 的宿主能力（`HostCapabilities.community_contribution`），鸿蒙版在实名认证到位之前为关；`startRequest` 据此拒绝社区的发布、改分类、评分和举报。
    CommunityContributionGate.adopt(client.hostCapabilities(PLATFORM));
    const seen = this.readStateFile(ONBOARDING_SEEN_FILE, 64);
    this.onboardingDone = false;
    if (seen !== null) {
      try { this.onboardingDone = (JSON.parse(seen) as OnboardingSeenValue).seen === true; }
      catch (_) { KeyboardLog.warn('onboarding seen marker is invalid'); }
    }
    SharedStateMirror.resolve(context).then((groupDir: string | null): void => {
      const sharedSandbox = HarmonySharedSandboxPolicy.status(groupDir);
      this.sharedSandboxAvailable = sharedSandbox.available;
      if (!sharedSandbox.available) {
        KeyboardLog.warn(`shared sandbox unavailable: ${sharedSandbox.reason}`);
        return;
      }
      this.sharedMirror = new SharedStateMirror(sharedSandbox.directory, context.filesDir);
      this.syncShared();
    });
    this.accountBridge = new HarmonyAccountCloudBridge(context);
    // The same copy the keyboard makes, by the same rule. The page reaches the dictionary through
    // it too, and two staging rules for one directory is how it came to hold a set matching neither.
    this.resourcesReady = StagedResources.stage(`${context.resourceDir}/engine`, `${context.filesDir}/engine`,
      `${context.filesDir}/engine.ready`);
    // Here as well as in the keyboard, so the page offers Cantonese, Zhuyin and Stroke (see hostCapabilities) before the keyboard has ever run.
    StagedResources.stageLanguageDictionaries(context.resourceDir, context.filesDir);
    StagedResources.stageHandwriting(context.resourceDir, context.filesDir);
  }


  /**
   * 设置窗口回到前台：先把自己的改动发布给键盘，再导入键盘在完整体验模式下发布的改动（键盘里改的偏好、打字统计）。导入的偏好由随后的 `refreshPreferencesIfChanged` 交给页面。键盘刷新过的账号会话由 Rust 在锁里决定要不要采用，这里排一次是为了让页面读到的账号状态尽早跟上；认证请求的顺序不靠它（网页回到可见时发的请求可能早于这个 ACTIVE 事件），每个请求自己经 `AccountSessionAdoption.beforeRequest` 再排一次（迁移差异 M2）。
   */
  syncShared(): void {
    if (this.sharedMirror === null) {
      return;
    }
    SharedStateSync.settingsSync(this.sharedMirror, this.stateDirectory);
    AccountSessionAdoption.adopt(this.sharedMirror, this.stateDirectory, 'keyboard');
    this.syncUserWords();
  }

  /** 立即把设置应用管的条目发布给键盘：窗口退到后台时调用，用户接下来多半要去别的应用打字。 */
  publishShared(): void {
    if (this.sharedPublishTimer !== -1) {
      clearTimeout(this.sharedPublishTimer);
      this.sharedPublishTimer = -1;
    }
    if (this.sharedMirror === null) {
      return;
    }
    SharedStateSync.settingsPublish(this.sharedMirror, this.stateDirectory);
    this.syncUserWords();
  }

  /**
   * 与键盘交换「我的词条」（`msime_client_user_words_mirror` 的 sync），在工作线程上：先把设置应用自己排进个人词库队列的词写进它的词库，再把键盘发布的改动（打字学到的词、候选栏里删掉的词）写进来，并把设置应用这边的改动（设置页改的词、导入的词、网盘同步写进来的词）发布给键盘。窗口回到前台和每次改完设置之后调用；本方日志、队列和键盘的发布从上次做完以后都没变时不做。键盘的改动写进来以后，词库同步要记一次本机改动，网盘才收得到。
   */
  private syncUserWords(): void {
    const mirror: SharedStateMirror | null = this.sharedMirror;
    if (mirror === null || this.context === undefined) return;
    if (this.userWordsRunning) {
      this.userWordsAgain = true;
      return;
    }
    const theirs: string = mirror.userWordsFile('keyboard');
    // 标记在开始之前取：这一次进行当中页面又改了词，下次标记不同就再做一次。
    const mark: string = [
      UserWordsMirror.journalMark(this.stateDirectory),
      UserWordsMirrorPolicy.mark(UserWordsMirrorPolicy.settingsQueueFiles(this.stateDirectory)
        .map((path: string): FileIdentity | null => UserWordsMirror.identity(path))),
      UserWordsMirror.fileMark(theirs),
    ].join('#');
    if (!UserWordsMirrorPolicy.settingsSyncDue(this.userWordsMark, mark)) return;
    let options: string;
    try {
      const prepared: Record<string, Object> = JSON.parse(client.prepareHost(JSON.stringify(
        KeyboardSession.prepareRequest(`${this.context.filesDir}/engine`, this.stateDirectory)))) as Record<string, Object>;
      if (prepared['ok'] !== true) {
        KeyboardLog.error(`user words not exchanged: host preparation refused: ${String(prepared['error'])}`);
        return;
      }
      options = JSON.stringify(prepared['value']);
    } catch (error) {
      KeyboardLog.error(`user words not exchanged: ${describeError(error)}`);
      return;
    }
    this.userWordsRunning = true;
    const request: string = UserWordsMirrorPolicy.request('sync', 'settings', options, this.stateDirectory, theirs,
      mirror.userWordsFile('settings'), SETTINGS_MAX_CHANGES);
    this.drainDictionaryQueues(options).then(async (drained: boolean): Promise<void> => {
      const reply: UserWordsReply = await UserWordsMirror.runAsync(request);
      this.userWordsRunning = false;
      if (drained && UserWordsMirrorPolicy.settled(reply)) {
        this.userWordsMark = mark;
      } else if (this.userWordsRetryTimer === -1) {
        this.userWordsRetryTimer = setTimeout((): void => {
          this.userWordsRetryTimer = -1;
          this.syncUserWords();
        }, SETTINGS_RETRY_MS);
      }
      if (UserWordsMirrorPolicy.dictionaryChanged(reply)) {
        this.userWordsChanged();
      }
      if (this.userWordsAgain) {
        this.userWordsAgain = false;
        this.syncUserWords();
      }
    });
  }

  /**
   * 设置应用自己排进个人词库队列的词（导入的个人词库文件、云词库下载、命名词库的增删），由设置应用自己写进它的词库，再经「我的词条」交换到键盘。API 12 起键盘排空的是它自己沙箱里的那个队列，这些词以前从来没进过任何一份词库。每轮写最多 4 条（队列的上限），再让命名词库送下一批；一次最多 `SETTINGS_QUEUE_ROUNDS` 轮，剩下的下次接着写。返回队列排空了没有。
   */
  private async drainDictionaryQueues(options: string): Promise<boolean> {
    try {
      for (let round = 0; round < SETTINGS_QUEUE_ROUNDS; round++) {
        const personal: string = await client.personalDictionarySyncAsync(options);
        const flushed: string = await client.dictionaryCollectionsAsync(
          `{"options":${options},"action":{"operation":"flush"}}`);
        if (UserWordsMirrorPolicy.queueDrained(personal, flushed)) return true;
      }
    } catch (error) {
      KeyboardLog.error(`personal dictionary queue not drained: ${describeError(error)}`);
    }
    return false;
  }

  /**
   * 设置应用的词库有了页面不知道的改动（键盘的词写了进来、排空队列写进了词）：告诉页面一声（`msime-user-words-changed`）。页面的词库同步（`use-dictionary-sync`）据此以 `local_change` 记一次本机改动，与经设置改了词走同一个队列；没有隐私政策的授权时页面拿不到词库同步、不听这个事件，同步下一次运行开头的核对照样会记下这些词。
   */
  private userWordsChanged(): void {
    this.webview?.runJavaScript("window.dispatchEvent(new Event('msime-user-words-changed'))").catch((): void => {});
  }

  /**
   * 改完设置后稍后发布给键盘。同步的桥接方法在开头调用它：发布在方法返回、改动落盘之后才执行。用户在设置页里点开一个输入框时键盘就会导入，所以不能只在窗口退到后台时发布。
   */
  private schedulePublish(): void {
    if (this.sharedMirror === null) {
      return;
    }
    if (this.sharedPublishTimer !== -1) {
      clearTimeout(this.sharedPublishTimer);
    }
    this.sharedPublishTimer = setTimeout((): void => {
      this.sharedPublishTimer = -1;
      this.publishShared();
    }, 300);
  }

  loadPreferences(): string {
    const reply: string = client.loadPreferences(this.stateDirectory);
    this.rememberRevision(reply);
    return PreferencesErrorCode.rewrite(reply);
  }

  /**
   * The revision the page was last handed, so a later activation can tell a changed document from
   * an unchanged one. Parsed defensively: a reply this cannot read leaves the previous value alone,
   * because a wrong number here would announce a change that did not happen.
   */
  private rememberRevision(reply: string): void {
    try {
      const parsed: RevisionReply = JSON.parse(reply) as RevisionReply;
      if (parsed.ok === true && parsed.value !== undefined && parsed.value !== null) {
        this.observedRevision =
          PreferenceRevisionPolicy.observe(this.observedRevision, parsed.value.revision);
      }
    } catch (error) {
      KeyboardLog.error(`preference revision unreadable: ${describeError(error)}`);
    }
  }

  /**
   * Hands the page the current document when it has moved since the page was given one.
   *
   * Called when the settings window comes back to the front, which on this host is the only moment
   * a cross-process change can be noticed: the keyboard writes preferences from its toolbar and
   * there is no listener between the two processes.
   */
  refreshPreferencesIfChanged(): void {
    const reply: string = client.loadPreferences(this.stateDirectory);
    let current: number = this.observedRevision;
    try {
      const parsed: RevisionReply = JSON.parse(reply) as RevisionReply;
      if (parsed.ok !== true || parsed.value === undefined || parsed.value === null) {
        return;
      }
      current = parsed.value.revision;
    } catch (error) {
      KeyboardLog.error(`preference revision unreadable: ${describeError(error)}`);
      return;
    }
    if (!PreferenceRevisionPolicy.changed(this.observedRevision, current)) {
      return;
    }
    this.observedRevision = current;
    // The page parks a listener under this name; calling it with the reply the page would have got
    // from loadPreferences keeps the two paths identical.
    if (this.webview === undefined) {
      return;
    }
    this.webview.runJavaScript(
      `globalThis.msimeHarmonyPreferencesChanged && globalThis.msimeHarmonyPreferencesChanged(${JSON.stringify(reply)})`)
      .catch((error: Object): void => {
        KeyboardLog.error(`preference change not delivered: ${describeError(error)}`);
      });
  }

  /**
   * Imports a skin folder the user picks.
   *
   * The source's button opens the client's skin folder so a skin can be dropped in. That folder is
   * inside the sandbox here and no file manager reaches it, so the direction is reversed: the user
   * points at a skin wherever it is and it is copied in. Same goal, and the only mechanism this
   * platform offers.
   *
   * A whole folder rather than a file, because a skin is a directory — the catalog looks for a
   * stylesheet inside one and refuses anything else, so importing a single file would import
   * something the catalog would then decline to list.
   */
  /**
   * Starts the skin import and answers at once.
   *
   * Like the picker, the page cannot wait for this: a bridge method that returns a Promise never
   * settles on this platform build, so an awaited import would leave the button reading 正在导入
   * forever. The panel's own rescan is what shows the result.
   */
  importSkinFolder(): string {
    this.runSkinImport().then((): void => {
      this.schedulePublish();
    }).catch((error: Object): void => {
      KeyboardLog.error(`skin import failed: ${describeError(error)}`);
    });
    return JSON.stringify({ ok: true, value: '', error: '' } as TextReply);
  }

  private async runSkinImport(): Promise<string> {
    if (this.context === undefined) {
      return JSON.stringify({ ok: false, error: 'settings context unavailable' } as TextReply);
    }
    let picked: string[] = [];
    try {
      const options: picker.DocumentSelectOptions = new picker.DocumentSelectOptions();
      if (canIUse('SystemCapability.FileManagement.UserFileService.FolderSelection')) {
        options.selectMode = picker.DocumentSelectMode.FOLDER;
      } else {
        throw new Error('folder_selection_unavailable');
      }
      options.maxSelectNumber = 1;
      picked = await new picker.DocumentViewPicker(this.context).select(options);
    } catch (error) {
      KeyboardLog.error(`skin picker refused: ${describeError(error)}`);
      return JSON.stringify({ ok: false, error: '无法打开选择器' } as TextReply);
    }
    if (picked.length === 0) {
      // Cancelling is not a failure, and reporting one would put an error on screen for a user who
      // simply changed their mind.
      return JSON.stringify({ ok: true, value: '', error: '' } as TextReply);
    }
    const name: string | null = SkinImportPolicy.pickedName(picked[0]);
    if (name === null) {
      KeyboardLog.error('skin folder name refused');
      return JSON.stringify({ ok: false, error: '皮肤文件夹名不可用' } as TextReply);
    }
    try {
      const skinsDirectory: string = `${this.stateDirectory}/skins`;
      if (!this.safeDirectory(this.stateDirectory)) {
        return JSON.stringify({ ok: false, error: '导入失败' });
      }
      if (!fs.accessSync(skinsDirectory)) fs.mkdirSync(skinsDirectory, true);
      if (!this.safeDirectory(skinsDirectory)) {
        return JSON.stringify({ ok: false, error: '导入失败' });
      }
      const reply: string = await client.skinImport(JSON.stringify({
        source: picked[0], directory: skinsDirectory
      }));
      const parsed: Record<string, Object> = JSON.parse(reply) as Record<string, Object>;
      if (parsed['ok'] !== true) {
        return JSON.stringify({ ok: false, error: '导入失败' } as TextReply);
      }
    } catch (error) {
      KeyboardLog.error(`skin import failed: ${describeError(error)}`);
      return JSON.stringify({ ok: false, error: '导入失败' } as TextReply);
    }
    KeyboardLog.info('skin imported');
    return JSON.stringify({ ok: true, value: name, error: '' } as TextReply);
  }

  /**
   * Starts one of the asynchronous requests and answers at once.
   *
   * A bridge method that returns a Promise never settles on this platform, so the ones below could
   * not answer the page at all: the account, the account-backed chat, the cloud dictionary and its
   * snapshots, the AI model list, the AI test and the credential test. They are left as they are
   * and started from here instead; the reply is delivered afterwards through `runJavaScript`, the
   * same direction the preference-change notification already uses.
   *
   * Chat is its own kind rather than another `account` caller because the page gives it a longer
   * deadline: a completion is a model writing text, and sharing the account kind would have meant
   * either cutting chat off early or letting a stalled profile fetch spin for two minutes.
   *
   * An unknown kind is answered rather than ignored. A page waiting on a number nobody will ever
   * send is indistinguishable from a slow network, and this one is a mistake that should say so.
   */
  startRequest(kind: string, id: number, payload: string): string {
    const deliver: (reply: string) => void = (reply: string): void => this.deliverReply(id, reply);
    // 账号、社区、云同步、检查更新、AI 和凭据测试这些请求要联网，没有隐私政策的授权一律拒绝；页面在没有授权时本来就不画这些入口，这里是第二道墙。
    if (PrivacyConsentPolicy.refuses(kind, payload, HarmonyPrivacyConsent.networkAllowed())) {
      KeyboardLog.info(`bridge request ${kind} refused: no privacy consent`);
      deliver(PrivacyConsentPolicy.refusal());
      return JSON.stringify({ ok: true, value: '', error: '' } as TextReply);
    }
    const failed: (error: Object) => void = (error: Object): void => {
      deliver(JSON.stringify({ ok: false, error: describeError(error) }));
    };
    if (!AiGenerationGate.allowsSettingsRequest(kind, payload)) {
      // 页面在总闸关着时本来就不画这些入口；这里再拦一道，旧页面或手写的请求也发不出去。
      deliver(AiGenerationGate.refusal());
    } else if (!CommunityContributionGate.allowsSettingsRequest(kind, payload)) {
      // 页面在总闸关着时本来就不画这些入口，`apps/harmony` 的社区客户端也先拒绝；这里再拦一道，旧页面或手写的请求也发不出去。
      deliver(CommunityContributionGate.refusal());
    } else if (kind === 'account') {
      this.account(payload).then(deliver).catch(failed);
    } else if (kind === 'cloud_dictionary') {
      this.cloudDictionary(payload).then(deliver).catch(failed);
    } else if (kind === 'chat') {
      this.account(payload).then(deliver).catch(failed);
    } else if (kind === 'cloud_clipboard') {
      this.cloudClipboard(payload).then(deliver).catch(failed);
    } else if (kind === 'cloud_clipboard_webdav_probe') {
      this.cloudClipboardWebDavProbe(payload).then(deliver).catch(failed);
    } else if (kind === 'dictionary_sync') {
      this.dictionarySync(payload).then(deliver).catch(failed);
    } else if (kind === 'settings_sync') {
      this.settingsSync(payload).then(deliver).catch(failed);
    } else if (kind === 'ai_skin') {
      this.aiSkin(id, payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`ai skin request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'ai_skin_unavailable' }));
      });
    } else if (kind === 'community_resource') {
      this.communityResource(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`community resource request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'community_unavailable' }));
      });
    } else if (kind === 'community_skin') {
      this.communitySkin(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`community skin request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'community_unavailable' }));
      });
    } else if (kind === 'community_plugin') {
      this.communityPlugin(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`community plugin request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'community_unavailable' }));
      });
    } else if (kind === 'community_block') {
      this.communityBlock(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`community block request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'community_unavailable' }));
      });
    } else if (kind === 'custom_skin_library') {
      this.customSkinLibrary(payload).then(deliver).catch(failed);
    } else if (kind === 'vocabulary_review') {
      this.vocabularyReview(payload).then(deliver).catch(failed);
    } else if (kind === 'read_aloud') {
      this.readAloud(payload).then(deliver).catch(failed);
    } else if (kind === 'repair_preferences') {
      this.repairPreferences().then(deliver).catch(failed);
    } else if (kind === 'dictionary_collections') {
      this.dictionaryCollections(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`dictionary collections request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'collections_io' }));
      });
    } else if (kind === 'dictionary_scel_text') {
      // 细胞词库转成标准文本：纯函数，不碰词库状态，也不需要宿主选项；文本随后照常经 dictionary 的 import 导入。
      client.dictionaryScelTextAsync(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`dictionary scel request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'dictionary_scel_invalid' }));
      });
    } else if (kind === 'statistics_auth_settings') {
      this.statisticsAuthenticationSettings(payload).then(deliver).catch(failed);
    } else if (kind === 'screen_capture_protection_settings') {
      this.screenCaptureProtectionSettings(payload).then(deliver).catch(failed);
    } else if (kind === 'statistics_auth') {
      if (this.context === undefined) {
        failed(new Error('认证宿主未就绪'));
      } else {
        this.statisticsAuthentication.authenticate(this.context).then((success: boolean): void => {
          deliver(success ? JSON.stringify({ ok: true, value: true }) : JSON.stringify({ ok: false, error: '未完成身份验证，请重试。' }));
        }).catch(failed);
      }
    } else if (kind === 'typing_statistics') {
      this.typingStatisticsOnWorker(payload).then(deliver).catch(failed);
    } else if (kind === 'typing_upload') {
      this.typingUpload(payload).then(deliver).catch(failed);
    } else if (kind === 'cloud_dictionary_snapshot') {
      this.cloudDictionarySnapshot(payload).then(deliver).catch(failed);
    } else if (kind === 'ai_models') {
      this.aiModels(payload).then(deliver).catch(failed);
    } else if (kind === 'ai_test') {
      this.aiTest(payload).then(deliver).catch(failed);
    } else if (kind === 'api_credential') {
      this.testApiCredential(payload).then(deliver).catch(failed);
    } else if (kind === 'voice_local_model') {
      this.voiceLocalModel(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`local voice model request failed: ${describeError(error)}`);
        deliver(LocalVoiceModelPolicy.refusal('local_model_failed'));
      });
    } else if (kind === 'helpcode_import') {
      this.importHelpcodeTable().then(deliver).catch(failed);
    } else if (kind === 'plugin_import') {
      this.importPlugin(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`plugin import failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'storage' }));
      });
    } else if (kind === 'candidate_skin_editor') {
      this.candidateSkinEditor(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`candidate skin editor request failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'storage' }));
      });
    } else if (kind === 'update_check') {
      this.updateCheck(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`update check failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'update_check_unavailable' }));
      });
    } else if (kind === 'save_export') {
      this.saveExport(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`export save failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'export_unavailable' }));
      });
    } else {
      KeyboardLog.error(`unknown bridge request: ${kind}`);
      this.deliverReply(id, JSON.stringify({ ok: false, error: 'unknown_request' }));
    }
    return JSON.stringify({ ok: true, value: '', error: '' } as TextReply);
  }

  /**
   * 皮肤页「制作皮肤」：`check` 按目录扫描的规则读一遍草稿（页面用它取外观补齐后的配色），`create` 用社区下载和 MCP 同一个安装器把草稿装进皮肤目录，规则全在 `msime_client_candidate_skin_editor`。皮肤根目录由这里填，页面发来的任何路径都不用；装好后像导入皮肤一样把新目录发布给键盘。
   */
  private async candidateSkinEditor(document: string): Promise<string> {
    const refused: string = JSON.stringify({ ok: false, error: 'candidate_skin_package' });
    let payload: CandidateSkinEditorPayload;
    try {
      payload = JSON.parse(document) as CandidateSkinEditorPayload;
    } catch {
      return refused;
    }
    const operation: string = typeof payload.operation === 'string' ? payload.operation : '';
    const packageId: string = typeof payload.package_id === 'string' ? payload.package_id : '';
    const manifest: string = typeof payload.manifest === 'string' ? payload.manifest : '';
    if ((operation !== 'check' && operation !== 'create') || packageId.length === 0 || manifest.length === 0) {
      return refused;
    }
    const files: Record<string, string> = payload.files ?? {};
    if (typeof files !== 'object') {
      return refused;
    }
    const request: CandidateSkinEditorRequest = {
      operation: operation,
      package_id: packageId,
      manifest: manifest,
      files: files,
      replace: payload.replace === true
    };
    if (operation === 'create') {
      if (!this.safeDirectory(this.stateDirectory)) {
        return JSON.stringify({ ok: false, error: 'storage' });
      }
      request.directory = `${this.stateDirectory}/skins`;
    }
    const reply: string = await client.candidateSkinEditor(JSON.stringify(request));
    if (operation === 'create') {
      try {
        const parsed: Record<string, Object> = JSON.parse(reply) as Record<string, Object>;
        if (parsed['ok'] === true) {
          this.schedulePublish();
        }
      } catch (error) {
        KeyboardLog.error(`candidate skin editor reply unreadable: ${describeError(error)}`);
      }
    }
    return reply;
  }

  /**
   * 「关于」页的检查更新，由原生工作线程上的 `msime_client_update_check` 回答，最多等 GitHub 十秒。平台在这里填，不取页面传来的值，页面只能问本宿主的发行版。
   */
  private async updateCheck(document: string): Promise<string> {
    let payload: UpdateCheckPayload;
    try {
      payload = JSON.parse(document) as UpdateCheckPayload;
    } catch {
      return JSON.stringify({ ok: false, error: 'update_check_invalid' });
    }
    if (typeof payload.current_version !== 'string') {
      return JSON.stringify({ ok: false, error: 'update_check_invalid' });
    }
    const request: UpdateCheckRequest = { platform: 'harmony', current_version: payload.current_version };
    if (typeof payload.edition === 'string') {
      request.edition = payload.edition;
    }
    if (typeof payload.arch === 'string') {
      request.arch = payload.arch;
    }
    return await client.updateCheck(JSON.stringify(request));
  }

  /**
   * Saves an exported dictionary where the user chooses, which is what the source's download link gets from WebView2.
   *
   * ArkWeb drops a blob download link unless a download delegate takes it, and a delegate would only save into the sandbox, where no file manager reaches. So the page hands the text over, it is staged privately, and the save picker decides where it goes, the same way the cloud dictionary export already works. The reply carries the saved file's name to show, or null when the user closed the picker, which is not a failure.
   */
  private async saveExport(document: string): Promise<string> {
    let request: ExportSaveRequest;
    try {
      request = JSON.parse(document) as ExportSaveRequest;
    } catch {
      return JSON.stringify({ ok: false, error: 'export_invalid' });
    }
    const name: string | null = SkinImportPolicy.destinationName(request.name);
    if (name === null || typeof request.contents !== 'string' || this.context === undefined) {
      return JSON.stringify({ ok: false, error: 'export_invalid' });
    }
    if (utf8Length(request.contents) > MAX_DICTIONARY_EXPORT_BYTES) {
      return JSON.stringify({ ok: false, error: 'export_too_large' });
    }
    const directory: string = `${this.stateDirectory}/dictionary-exports`;
    const staging: string = `${directory}/${util.generateRandomUUID(false)}.txt`;
    let destination: string = '';
    try {
      if (!this.safeDirectory(this.stateDirectory)) {
        return JSON.stringify({ ok: false, error: 'export_storage' });
      }
      if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);
      if (!this.safeDirectory(directory) || this.symbolicLink(staging)) {
        return JSON.stringify({ ok: false, error: 'export_storage' });
      }
      const file: fs.File = fs.openSync(staging, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        await fs.write(file.fd, request.contents);
      } finally {
        fs.closeSync(file);
      }
      const options: picker.DocumentSaveOptions = new picker.DocumentSaveOptions();
      options.newFileNames = [name];
      const selected: string[] = await new picker.DocumentViewPicker(this.context).save(options);
      if (selected.length === 0) {
        return JSON.stringify({ ok: true, value: null, error: '' });
      }
      destination = selected[0];
      await fs.copyFile(staging, destination);
      KeyboardLog.info('export saved');
      // The picker answers with a provider URI, which means nothing to the user; the file's own name is what they will look for.
      return JSON.stringify({ ok: true, value: SkinImportPolicy.pickedName(destination) ?? name, error: '' });
    } catch (error) {
      KeyboardLog.error(`export save failed: ${describeError(error)}`);
      try {
        if (destination.length > 0 && fs.accessSync(destination)) {
          fs.unlinkSync(destination);
        }
      } catch {
        // Reporting the failure matters more than a picker URI which has already disappeared.
      }
      return JSON.stringify({ ok: false, error: 'export_storage' });
    } finally {
      try {
        if (fs.accessSync(staging)) {
          fs.unlinkSync(staging);
        }
      } catch {
        // A missing staging file is already the desired postcondition.
      }
    }
  }

  /**
   * The on-device model manager: list, install, cancel and remove catalog models under `<filesDir>/voice-models`, the directory the keyboard's local recognizer loads `asr_model_path` from.
   *
   * Every operation goes through the asynchronous bridge because an install blocks for the whole download and runs on a native worker; the three short ones answer at once but share the route so the page has one shape to wait on. The install downloads through the mirror the saved preferences name, read at the moment it starts, which is what the page means by "save the settings first".
   */
  private async voiceLocalModel(document: string): Promise<string> {
    const action: LocalVoiceModelAction | null = LocalVoiceModelPolicy.action(document);
    if (action === null || this.context === undefined) {
      return LocalVoiceModelPolicy.refusal('local_model_failed');
    }
    const root: string = LocalVoiceModelPolicy.root(this.context.filesDir);
    const id: string = action.id ?? '';
    if (action.operation === 'list') {
      const request: LocalVoiceModelRequest = { root: root };
      return LocalVoiceModelPolicy.listReply(client.voiceLocalModels(JSON.stringify(request)), root);
    }
    if (action.operation === 'cancel') {
      const request: LocalVoiceModelRequest = { id: id };
      const reply: string = LocalVoiceModelPolicy.rewrite(client.voiceLocalModelCancel(JSON.stringify(request)));
      if (!this.voiceImportsPending.has(id)) {
        return reply;
      }
      // 导入还在复制文件，原生安装没开始：记下取消，复制循环看到后不再安装。
      this.voiceImportsCancelled.add(id);
      return JSON.stringify({ ok: true, value: true } as FlagReply);
    }
    if (action.operation === 'remove') {
      const request: LocalVoiceModelRequest = { root: root, id: id };
      return LocalVoiceModelPolicy.rewrite(
        await client.voiceLocalModelRemove(JSON.stringify(request)));
    }
    try {
      if (!fs.accessSync(root)) fs.mkdirSync(root, true);
    } catch {
      KeyboardLog.error('voice model storage unavailable');
      return LocalVoiceModelPolicy.refusal('local_model_failed');
    }
    if (action.operation === 'import') {
      this.voiceImportsPending.add(id);
      this.voiceImportsCancelled.delete(id);
      try {
        return await this.importVoiceModel(root, id);
      } finally {
        this.voiceImportsPending.delete(id);
        this.voiceImportsCancelled.delete(id);
      }
    }
    const mirror: string = LocalVoiceModelPolicy.mirror(client.loadPreferences(this.stateDirectory));
    const request: LocalVoiceModelRequest = { root: root, id: id, mirror: mirror };
    const reply: string = await client.voiceLocalModelInstall(JSON.stringify(request),
      (progress: string): void => this.deliverVoiceModelProgress(progress));
    return LocalVoiceModelPolicy.installReply(reply);
  }

  /**
   * 「从文件导入」：用系统选择器让用户多选自己下载好的模型文件，不联网安装。
   *
   * 选择器给的是原生库打不开的文档 URI，所以先把文件复制进私有缓存目录，用固定的序号作文件名，再把这些路径交给和下载同一个 `msime_client_voice_local_model_install`（带 `files`），由 client-providers 按长度和 SHA-256 认文件、校验、解压并发布，和其他宿主完全相同。长度和目录里任何一个需要的文件都对不上的选择不复制。缓存副本不论成败都在安装结束后删掉。用户关掉选择器时回答 null，不算失败。
   */
  private async importVoiceModel(root: string, id: string): Promise<string> {
    try {
      const context: common.UIAbilityContext | undefined = this.context;
      if (context === undefined) {
        return LocalVoiceModelPolicy.refusal('local_model_failed');
      }
      let picked: string[] = [];
      try {
        const options: picker.DocumentSelectOptions = new picker.DocumentSelectOptions();
        if (canIUse('SystemCapability.FileManagement.UserFileService.FolderSelection')) {
          options.selectMode = picker.DocumentSelectMode.FILE;
        }
        options.maxSelectNumber = MAX_VOICE_MODEL_IMPORT_PICKS;
        picked = await new picker.DocumentViewPicker(context).select(options);
      } catch (error) {
        KeyboardLog.error(`voice model picker refused: ${describeError(error)}`);
        return LocalVoiceModelPolicy.refusal('local_model_failed');
      }
      if (picked.length === 0) {
        return JSON.stringify({ ok: true, value: null, error: '' });
      }
      if (this.voiceImportsCancelled.has(id)) {
        return LocalVoiceModelPolicy.refusal('local_model_cancelled');
      }
      const listRequest: LocalVoiceModelRequest = { root: root };
      const sizes: number[] = LocalVoiceModelPolicy.importSizes(
        client.voiceLocalModels(JSON.stringify(listRequest)), id);
      const importRoot: string = `${context.cacheDir}/voice-model-import`;
      const staging: string = `${importRoot}/${util.generateRandomUUID(false)}`;
      try {
        if (!this.safeDirectory(context.cacheDir)) {
          return LocalVoiceModelPolicy.refusal('local_model_io');
        }
        if (!fs.accessSync(importRoot)) fs.mkdirSync(importRoot, true);
        if (!this.safeDirectory(importRoot)) {
          return LocalVoiceModelPolicy.refusal('local_model_io');
        }
        fs.mkdirSync(staging, true);
        if (!this.safeDirectory(staging)) {
          return LocalVoiceModelPolicy.refusal('local_model_io');
        }
        const files: string[] = [];
        for (const uri of picked) {
          if (this.voiceImportsCancelled.has(id)) {
            return LocalVoiceModelPolicy.refusal('local_model_cancelled');
          }
          const file: fs.File = fs.openSync(uri, fs.OpenMode.READ_ONLY);
          try {
            const opened: fs.Stat = fs.statSync(file.fd);
            if (!opened.isFile() || !sizes.includes(opened.size)) {
              continue;
            }
            const copy: string = `${staging}/${files.length}`;
            await fs.copyFile(file.fd, copy);
            files.push(copy);
          } finally {
            fs.closeSync(file);
          }
        }
        if (this.voiceImportsCancelled.has(id)) {
          return LocalVoiceModelPolicy.refusal('local_model_cancelled');
        }
        const request: LocalVoiceModelRequest = { root: root, id: id, files: files };
        const reply: string = await client.voiceLocalModelInstall(JSON.stringify(request),
          (progress: string): void => this.deliverVoiceModelProgress(progress));
        return LocalVoiceModelPolicy.installReply(reply);
      } finally {
        try {
          StagedResources.removeDirectory(staging);
        } catch (error) {
          KeyboardLog.error(`voice model import staging not removed: ${describeError(error)}`);
        }
      }
    } catch (error) {
      KeyboardLog.error('pages.Settings.importVoiceModel failed');
      throw error as Error;
    }
  }

  private deliverVoiceModelProgress(progress: string): void {
    if (this.webview === undefined) return;
    this.webview.runJavaScript(
      `globalThis.msimeHarmonyVoiceModelProgress && globalThis.msimeHarmonyVoiceModelProgress(`
      + `${JSON.stringify(progress)})`)
      .catch((error: Object): void => {
        // A progress tick that does not arrive costs the user a moving bar, not the download.
        KeyboardLog.error(`voice model progress not delivered: ${describeError(error)}`);
      });
  }

  /** Hands one reply back to the page, under the number it was asked with. */
  private deliverReply(id: number, reply: string): void {
    // 异步请求（账号、社区皮肤、设置同步等）都经这里交回结果，它们可能改了要发布给键盘的文件。
    this.schedulePublish();
    if (this.webview === undefined) {
      return;
    }
    this.webview.runJavaScript(
      `globalThis.msimeHarmonyBridgeReply && globalThis.msimeHarmonyBridgeReply(${id}, `
      + `${JSON.stringify(reply)})`)
      .catch((error: Object): void => {
        KeyboardLog.error(`bridge reply not delivered: ${describeError(error)}`);
      });
  }

  /**
   * The 插件 page's pack store, plugin editor and @ name list: `catalog`, `remove`, `load_mentions`, `save_mentions`, `authoring_load` and `authoring_save`, answered by `msime_client_plugins` with the same rules and failure codes the desktop shell answers with. The page never names a path: the state root and the bundle's built-in sound packs are filled in here, and an import, which has to wait for the system picker, arrives through `startRequest` as `plugin_import` instead.
   */
  plugins(actionDocument: string): string {
    this.schedulePublish();
    const refused: string = JSON.stringify({ ok: false, error: 'invalid' });
    if (this.context === undefined) {
      return refused;
    }
    let action: PluginPageAction;
    try {
      action = JSON.parse(actionDocument) as PluginPageAction;
    } catch {
      return refused;
    }
    if (!['catalog', 'remove', 'load_mentions', 'save_mentions', 'authoring_load', 'authoring_save']
      .includes(action.operation)) {
      return refused;
    }
    return client.plugins(`{"state_root":${JSON.stringify(this.stateDirectory)},`
      + `"sound_packs":${JSON.stringify(this.soundPacks())},"action":${actionDocument}}`);
  }

  /** The built-in sound packs this bundle ships, where the keyboard's players read them too. */
  private soundPacks(): string {
    return this.context === undefined ? '' : `${this.context.resourceDir}/sound-packs`;
  }

  /** 原始字节有界读取、严格 UTF-8 解码；码表格式与原子安装仍由共享 Rust 负责。 */
  private async importHelpcodeTable(): Promise<string> {
    const context: common.UIAbilityContext | undefined = this.context;
    if (context === undefined) return JSON.stringify({ ok: false, error: 'storage' });
    let handle: fs.File | null = null;
    try {
      const options: picker.DocumentSelectOptions = new picker.DocumentSelectOptions();
      options.maxSelectNumber = 1;
      options.fileSuffixFilters = ['辅助码表|.txt'];
      const picked: string[] = await new picker.DocumentViewPicker(context).select(options);
      if (picked.length === 0) return JSON.stringify({ ok: true, value: null });
      const expected: fs.Stat = fs.lstatSync(picked[0]);
      if (expected.isSymbolicLink() || !expected.isFile()) {
        return JSON.stringify({ ok: false, error: 'plugin_invalid', detail: '辅助码表不是普通文件' });
      }
      handle = fs.openSync(picked[0], fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      const maximum: number = 1024 * 1024;
      if (!stat.isFile() || stat.ino !== expected.ino || stat.size < 1 || stat.size > maximum) {
        return JSON.stringify({ ok: false, error: 'plugin_invalid', detail: '辅助码表为空、超过 1 MiB 或文件已变化' });
      }
      // 多读一个字节识别读取期间增长；分段读取避免把合法短读误认为完整文件。
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      let count: number = 0;
      while (count < buffer.byteLength) {
        const chunk: ArrayBuffer = new ArrayBuffer(buffer.byteLength - count);
        const read: number = fs.readSync(handle.fd, chunk);
        if (read === 0) break;
        new Uint8Array(buffer).set(new Uint8Array(chunk, 0, read), count);
        count += read;
      }
      if (count !== stat.size) {
        return JSON.stringify({ ok: false, error: 'plugin_invalid', detail: '读取辅助码表期间文件大小发生变化' });
      }
      let text: string;
      try {
        text = util.TextDecoder.create('utf-8', { fatal: true }).decodeToString(new Uint8Array(buffer, 0, count));
      } catch {
        return JSON.stringify({ ok: false, error: 'plugin_invalid', detail: '辅助码表必须是 UTF-8 文本' });
      }
      const action: HelpcodeImportAction = {
        operation: 'import_helpcode_table', file_name: new fileUri.FileUri(picked[0]).name, text: text
      };
      const call: HelpcodeImportCall = { state_root: this.stateDirectory, action: action };
      return await client.pluginsAsync(JSON.stringify(call));
    } catch {
      // 不记录文档 URI、文件名或码表正文。
      return JSON.stringify({ ok: false, error: 'storage' });
    } finally {
      if (handle !== null) NativeCleanup.close(handle);
    }
  }

  /**
   * 安装用户用系统选择器选中的包目录或 `.zip` 文件。
   *
   * 选择器交回的是原生库打不开的文档 URI，所以先把选中的内容复制进私有暂存目录，用固定的名字而不是选中时的名字，再在工作线程上由 `msime_client_plugins` 从那里安装；它按与其他所有宿主相同的规则检查，拒绝时不留下任何已安装的内容。复制到 client-services 的上限（`PluginImportPolicy`）就停止，因此选错的文件在被整份复制之前就会被拒绝。不论导入怎样结束，暂存副本都会在工作线程用完之后删掉。回答是安装好的包；用户关掉选择器时回答 null，不算失败。
   */
  private async importPlugin(document: string): Promise<string> {
    const context: common.UIAbilityContext | undefined = this.context;
    let request: PluginImportRequest;
    try {
      request = JSON.parse(document) as PluginImportRequest;
    } catch {
      return JSON.stringify({ ok: false, error: 'invalid' });
    }
    if (context === undefined || (request.source !== 'folder' && request.source !== 'archive')) {
      return JSON.stringify({ ok: false, error: 'invalid' });
    }
    const folder: boolean = request.source === 'folder';
    let picked: string[] = [];
    try {
      const options: picker.DocumentSelectOptions = new picker.DocumentSelectOptions();
      if (canIUse('SystemCapability.FileManagement.UserFileService.FolderSelection')) {
        options.selectMode = folder ? picker.DocumentSelectMode.FOLDER : picker.DocumentSelectMode.FILE;
      } else if (folder) {
        throw new Error('folder_selection_unavailable');
      }
      options.maxSelectNumber = 1;
      if (!folder) {
        options.fileSuffixFilters = ['插件|.zip'];
      }
      picked = await new picker.DocumentViewPicker(context).select(options);
    } catch (error) {
      KeyboardLog.error(`plugin picker refused: ${describeError(error)}`);
      return JSON.stringify({ ok: false, error: 'storage' });
    }
    if (picked.length === 0) {
      return JSON.stringify({ ok: true, value: null });
    }
    const staging: string = `${context.cacheDir}/plugin-import/${util.generateRandomUUID(false)}`;
    try {
      const pluginRoot: string = `${context.cacheDir}/plugin-import`;
      if (!this.safeDirectory(context.cacheDir)) {
        return JSON.stringify({ ok: false, error: 'storage' });
      }
      if (!fs.accessSync(pluginRoot)) fs.mkdirSync(pluginRoot, true);
      if (!this.safeDirectory(pluginRoot)) {
        return JSON.stringify({ ok: false, error: 'storage' });
      }
      fs.mkdirSync(staging, true);
      if (!this.safeDirectory(staging)) {
        return JSON.stringify({ ok: false, error: 'storage' });
      }
      let source: string;
      if (folder) {
        source = `${staging}/${PluginImportPolicy.FOLDER_NAME}`;
        const refusal: PluginImportRefusal | null =
          await this.copyPluginFolder(new fileUri.FileUri(picked[0]).path, source);
        if (refusal !== null) {
          return JSON.stringify(refusal);
        }
      } else {
        source = `${staging}/${PluginImportPolicy.ARCHIVE_NAME}`;
        const pathStat: fs.Stat = fs.lstatSync(picked[0]);
        if (pathStat.isSymbolicLink() || !pathStat.isFile()) {
          return JSON.stringify({ ok: false, error: 'plugin_invalid', detail: '压缩包不是普通文件' });
        }
        const file: fs.File = fs.openSync(picked[0], fs.OpenMode.READ_ONLY);
        try {
          const opened: fs.Stat = fs.statSync(file.fd);
          if (!opened.isFile() || opened.ino !== pathStat.ino) {
            return JSON.stringify({ ok: false, error: 'plugin_invalid', detail: '压缩包文件发生变化' });
          }
          const refusal: PluginImportRefusal | null = PluginImportPolicy.archive(fs.statSync(file.fd).size);
          if (refusal !== null) {
            return JSON.stringify(refusal);
          }
          await fs.copyFile(file.fd, source);
        } finally {
          fs.closeSync(file);
        }
      }
      const action: PluginImportAction = { operation: 'import', source: source };
      const call: PluginImportCall = {
        state_root: this.stateDirectory, sound_packs: this.soundPacks(), action: action
      };
      const reply: string = await client.pluginsAsync(JSON.stringify(call));
      KeyboardLog.info('plugin import answered');
      return reply;
    } catch (error) {
      KeyboardLog.error(`plugin import failed: ${describeError(error)}`);
      return JSON.stringify({ ok: false, error: 'storage' });
    } finally {
      try {
        StagedResources.removeDirectory(staging);
      } catch (error) {
        KeyboardLog.error(`plugin import staging not removed: ${describeError(error)}`);
      }
    }
  }

  /**
   * Copy the top level of a picked pack folder to `destination`, or answer why it cannot be a pack without copying anything. Every entry is looked at before the first byte is copied, and the look stops at the first entry that rules the folder out: a subfolder, a link, or more files or bytes than any pack may hold.
   */
  private async copyPluginFolder(picked: string, destination: string): Promise<PluginImportRefusal | null> {
    try {
      const scan: PluginFolderScan = new PluginFolderScan();
      for (const name of fs.listFileSync(picked)) {
        if (PluginImportPolicy.hidden(name)) {
          continue;
        }
        const stat: fs.Stat = fs.lstatSync(`${picked}/${name}`);
        const kind: PickedEntryKind = stat.isSymbolicLink() ? PickedEntryKind.LINK
          : stat.isDirectory() ? PickedEntryKind.DIRECTORY
          : stat.isFile() ? PickedEntryKind.FILE : PickedEntryKind.OTHER;
        const refusal: PluginImportRefusal | null = scan.take(name, kind, stat.size);
        if (refusal !== null) {
          return refusal;
        }
      }
      fs.mkdirSync(destination);
      for (const name of scan.files) {
        const sourcePath: string = `${picked}/${name}`;
        const destinationPath: string = `${destination}/${name}`;
        const file: fs.File = fs.openSync(sourcePath, fs.OpenMode.READ_ONLY);
        try {
          const pathStat: fs.Stat = fs.lstatSync(sourcePath);
          const opened: fs.Stat = fs.statSync(file.fd);
          if (pathStat.isSymbolicLink() || !pathStat.isFile()
            || !opened.isFile() || opened.ino !== pathStat.ino) {
            return { ok: false, error: 'plugin_invalid', detail: `${name} 文件发生变化` };
          }
          await fs.copyFile(file.fd, destinationPath);
        } finally {
          fs.closeSync(file);
        }
      }
      return null;
    } catch (error) {
      KeyboardLog.error('pages.Settings.copyPluginFolder failed');
      throw error as Error;
    }
  }

  /** The shared settings page uses the same validated catalog as desktop presenters. */
  scanSkinCatalog(): string {
    const directory: string = `${this.stateDirectory}/skins`;
    const raw: string = client.skinCatalog(directory);
    try {
      const response: Object = JSON.parse(raw) as Object;
      if ((response as Record<string, Object>)['ok'] === true) {
        const value: Record<string, Object> =
          (response as Record<string, Object>)['value'] as Record<string, Object>;
        value['directory'] = directory;
      }
      return JSON.stringify(response);
    } catch {
      return raw;
    }
  }

  /**
   * AI skin generation, which is the one request that reports before it answers.
   *
   * Three pictures take minutes, so a run that said nothing until it finished would be
   * indistinguishable from one that had stopped. Progress goes out on its own channel rather than
   * as part of the reply, the same direction the preference-change notification uses, and the
   * request id the page chose travels with it so a stale run's counter cannot drive a new one's
   * display.
   */
  async aiSkin(requestId: number, actionDocument: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return JSON.stringify({ ok: false, error: 'ai_skin_unavailable' });
    }
    let action: AiSkinAction;
    try {
      action = JSON.parse(actionDocument) as AiSkinAction;
    } catch {
      return JSON.stringify({ ok: false, error: 'ai_skin_invalid' });
    }
    const id: string = typeof action.request_id === 'string' ? action.request_id : '';
    return await this.accountBridge.aiSkin(actionDocument, (completed: number): void => {
      this.deliverAiSkinProgress(id, completed);
    });
  }

  private deliverAiSkinProgress(requestId: string, completed: number): void {
    if (this.webview === undefined) return;
    this.webview.runJavaScript(
      `globalThis.msimeHarmonyAiSkinProgress && globalThis.msimeHarmonyAiSkinProgress(`
      + `${JSON.stringify(requestId)}, ${completed})`)
      .catch((error: Object): void => {
        // A progress tick that does not arrive costs the user a moving number, not the run.
        KeyboardLog.error(`ai skin progress not delivered: ${describeError(error)}`);
      });
  }

  /**
   * Community dictionaries and reply templates.
   *
   * Everything but the two library operations is one request to the account bridge. Keeping and
   * forgetting a reply template are not requests at all: that library is a local file, and it is
   * the only thing the settings surface and the keyboard process share about the community.
   *
   * The file sits beside the state directory rather than inside it, at `files/CommunityLibrary.json`
   * — the same path `KeyboardSession.communityReplyTemplates` reads. The keyboard extension is given
   * `files/state`, so a library written inside that directory would be written where the reader
   * never looks.
   */
  async communityResource(actionDocument: string): Promise<string> {
    if (this.accountBridge === undefined || this.context === undefined) {
      return JSON.stringify({ ok: false, error: 'community_unavailable' });
    }
    let action: CommunityResourceAction;
    try {
      action = JSON.parse(actionDocument) as CommunityResourceAction;
    } catch {
      return JSON.stringify({ ok: false, error: 'community_invalid' });
    }
    const file: string = `${this.context.filesDir}/CommunityLibrary.json`;
    if (action.operation === 'store_reply') {
      return await client.communityResourceLibrary(`{"file":${JSON.stringify(file)},`
        + `"action":{"operation":"save_reply","item":${JSON.stringify(action.item ?? null)}}}`);
    }
    if (action.operation === 'remove_reply') {
      const remove: ResourceLibraryRequest = {
        file: file,
        action: { operation: 'remove', id: action.id ?? '' } as ResourceLibraryRemove
      };
      return await client.communityResourceLibrary(JSON.stringify(remove));
    }
    return this.accountBridge.community(actionDocument);
  }

  /**
   * 社区皮肤图库。
   *
   * 浏览、评分、发布、下架和「获取」都交给 Rust（`msime_client_community`）；评分和发布在社区投稿类功能的总闸关着时已在 `startRequest` 被拒绝。「获取」（`keyboard_skin_install`）下载设计、开始试用并导入皮肤库是一步，导入失败时结束刚开始的试用；它改写了偏好文档，所以之后把新文档交给页面。
   *
   * 结束试用不会联网；它响应已应用皮肤的「保留」或「还原」，直接在设备上处理。
   */
  async communitySkin(actionDocument: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return JSON.stringify({ ok: false, error: 'community_unavailable' });
    }
    let action: CommunitySkinAction;
    try {
      action = JSON.parse(actionDocument) as CommunitySkinAction;
    } catch {
      return JSON.stringify({ ok: false, error: 'community_invalid' });
    }
    if (action.operation === 'finish_trial') {
      return this.finishSkinTrial(action);
    }
    const reply: string = await this.accountBridge.community(actionDocument);
    if (action.operation === 'keyboard_skin_install') {
      // 装好的皮肤写进了偏好文档，页面手里那份已经旧了；理由与设置同步相同，旧的控件会把下一次保存当作冲突拒绝。
      this.refreshPreferencesIfChanged();
    }
    return reply;
  }

  /**
   * 社区插件图库：plugin_* 操作原样交给 Rust（`msime_client_community`），打包预检、发布和安装由 `communityRequest` 带上状态目录。鸿蒙的 Rust 没有脚本插件这个类型，这里只会经手数据类插件。
   */
  async communityPlugin(actionDocument: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return JSON.stringify({ ok: false, error: 'community_unavailable' });
    }
    return await this.accountBridge.community(actionDocument);
  }

  /**
   * 社区的屏蔽作者：作品详情里的「屏蔽作者」和「隐私」页的「已屏蔽的作者」。`block`、`block_list`、`unblock` 原样交给 Rust（`msime_client_community`），只认登录账号；不是投稿，社区投稿类功能的总闸关着时照常放行。
   */
  async communityBlock(actionDocument: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return JSON.stringify({ ok: false, error: 'community_unavailable' });
    }
    return await this.accountBridge.communityBlock(actionDocument);
  }

  private finishSkinTrial(action: CommunitySkinAction): string {
    const finish: SkinTrialRequest = {
      directory: this.stateDirectory,
      action: { operation: 'finish', id: action.id, keep: action.keep === true } as SkinTrialFinish
    };
    const reply: string = client.keyboardSkinTrial(JSON.stringify(finish));
    this.refreshPreferencesIfChanged();
    return reply;
  }

  /**
   * The user's own named touch-keyboard designs.
   *
   * Forwarded to the shared store rather than kept here. That store already owns the file lock, the
   * atomic replace, the twelve-item limit and the name rules; a second implementation on this host
   * would be a second opinion about what is in the library, and the two would drift the first time
   * either side changed a rule.
   *
   * The state directory comes from the host, not from the page: where the library lives is not
   * something a WebView should be able to name.
   */
  private async customSkinLibrary(action: string): Promise<string> {
    const request: string = action.length === 0
      ? JSON.stringify({ directory: this.stateDirectory })
      : `{"directory":${JSON.stringify(this.stateDirectory)},"action":${action}}`;
    return await client.customSkinLibrary(request);
  }

  /** 复用键盘已有的无状态计算，网页不另存方案表或校验规则。 */
  nineKeyShuangpinLayout(request: string): string {
    return client.nineKeyShuangpinLayout(request);
  }

  /**
   * Which packaged dictionary is installed.
   *
   * Read from the staged engine resources rather than from the module's rawfile: staging is what
   * verifies them against the pinned lock, and the copy the keyboard actually runs on is the one
   * worth reporting. The page is given the two answers, never the path.
   */
  dictionaryManifest(): string {
    if (this.context === undefined) {
      return JSON.stringify({ ok: false, error: 'dictionary_manifest_unavailable' });
    }
    return client.dictionaryManifest(`${this.context.filesDir}/engine`);
  }

  /** Read only an image declared by a validated external skin manifest, as base64: the default `bytes` array costs the shared layer a JSON value per byte, about 512 MiB at the 8 MiB resource limit. The page side turns it back into bytes. */
  readSkinImage(id: string, relative: string): string {
    return client.skinResource(JSON.stringify({
      directory: `${this.stateDirectory}/skins`, id: id, relative: relative, kind: 'image', encoding: 'base64'
    }));
  }

  /** Read only a font declared by a validated external skin manifest, as base64 for the same reason as `readSkinImage`. */
  readSkinFont(id: string, relative: string): string {
    return client.skinResource(JSON.stringify({
      directory: `${this.stateDirectory}/skins`, id: id, relative: relative, kind: 'font', encoding: 'base64'
    }));
  }

  /** Read the package-declared toolbar stylesheet without exposing a filesystem path. */
  readSkinToolbarCss(id: string): string {
    return client.skinToolbarStylesheet(JSON.stringify({
      directory: `${this.stateDirectory}/skins`, id: id
    }));
  }

  /**
   * The revision is what the page read, not what it is writing: the store compares it against what
   * is on disk and refuses the save if the keyboard changed something in between, which is the whole
   * point of having one.
   */
  /**
   * 设置页的「修复配置文件…」：只在用户点了之后才做。读不了的文件（多半是更新的版本写的）先原样备份在旁边，再按本版本认得的设置重写，其余取默认。修复后的文档和读取一样记下版本号，页面拿到的就是新的那份。
   */
  private async repairPreferences(): Promise<string> {
    const reply: string = await client.repairPreferences(this.stateDirectory);
    try {
      const parsed: RepairReply = JSON.parse(reply) as RepairReply;
      if (parsed.ok === true && parsed.value !== undefined) {
        this.observedRevision = PreferenceRevisionPolicy.observe(this.observedRevision,
          parsed.value.snapshot.revision);
      }
    } catch (error) {
      KeyboardLog.error(`preference repair reply unreadable: ${describeError(error)}`);
    }
    return PreferencesErrorCode.rewrite(reply);
  }

  savePreferences(expectedRevision: number, document: string): string {
    this.schedulePublish();
    const reply: string = client.savePreferences(this.stateDirectory, expectedRevision, document);
    this.rememberRevision(reply);
    // Turning usage reporting off drops what is queued at once rather than at the next start; the keyboard's running session goes with it.
    if (TelemetryPolicy.reportingTurnedOff(document, reply)) {
      HarmonyTelemetry.reportingTurnedOff(this.stateDirectory);
    }
    return PreferencesErrorCode.rewrite(reply);
  }

  /**
   * What this host can do, plus the one field the shared client leaves to the host to fill.
   *
   * `os_version` is deliberately not part of `HostCapabilities::for_platform` — it is not a
   * platform assumption but a fact about the machine, so each host reads it and adds it. Without
   * it the feedback page names the platform and quotes the web view's user agent, which identifies
   * the browser engine rather than the system a report would have to be reproduced on.
   */
  /** Whether an input scheme's dictionary, if it needs one, is in the staged language-dictionaries directory. */
  private languageDictionaryInstalled(scheme: string): boolean {
    try {
      const file: string | null = KeyboardScheme.languageDictionary(scheme);
      return file === null
        || (this.context !== undefined && fs.accessSync(`${this.context.filesDir}/language-dictionaries/${file}`));
    } catch (error) {
      KeyboardLog.error('pages.Settings.languageDictionaryInstalled failed');
      return false;
    }
  }

  /** 设备能不能折叠：可折叠手机展开后是大屏，设置页才给出大屏单手的侧边数字键盘。显示服务查不到时按不能折叠，少列一项设置而不是让整份能力文档失败。 */
  private foldable(): boolean {
    try {
      return display.isFoldable();
    } catch (error) {
      KeyboardLog.error(`foldable state unavailable: ${describeError(error)}`);
      return false;
    }
  }

  hostCapabilities(): string {
    const document: string = client.hostCapabilities(PLATFORM);
    const release: string | null = OsVersionPolicy.release(deviceInfo.osFullName);
    try {
      const reply: Record<string, Object> = JSON.parse(document) as Record<string, Object>;
      if (reply['ok'] !== true) {
        return document;
      }
      const value: Record<string, Object> = reply['value'] as Record<string, Object>;
      // Unlike the platform name, the form factor is not fixed: this HAP runs on both phones and
      // 2-in-1 machines. Keep phone navigation on touch devices and give a 2-in-1 the large-screen
      // settings surface that matches its physical keyboard and candidate panel.
      const projection: SettingsFormFactorProjection =
        SettingsFormFactorCapabilities.resolve(deviceInfo.deviceType, this.foldable());
      value['mobile_settings'] = projection.mobileSettings;
      value['panel_windows'] = projection.panelWindows;
      value['floating_toolbar'] = projection.floatingToolbar;
      value['floating_toolbar_appearance'] = projection.floatingToolbarAppearance;
      value['floating_toolbar_components'] = projection.floatingToolbarComponents;
      value['floating_toolbar_handwriting'] = projection.floatingToolbarHandwriting;
      value['floating_toolbar_voice'] = projection.floatingToolbarVoice;
      value['mode_switch_shortcuts'] = projection.modeSwitchShortcuts;
      value['panel_shortcuts'] = projection.panelShortcuts;
      value['number_row_selection'] = projection.numberRowSelection;
      value['voice_hotkeys'] = projection.voiceHotkeys;
      if (projection.fixedCandidateLayout !== null) {
        value['fixed_candidate_layout'] = projection.fixedCandidateLayout;
      }
      value['candidate_follow_cursor'] = projection.candidateFollowCursor;
      value['candidate_window_scale'] = projection.candidateWindowScale;
      value['candidate_window_opacity'] = projection.candidateWindowOpacity;
      value['candidate_corner_radius'] = projection.candidateCornerRadius;
      value['input_mode_hud'] = projection.inputModeHud;
      value['key_sound'] = projection.keySound;
      value['music'] = projection.music;
      value['plugin_triggers'] = projection.pluginTriggers;
      value['typing_effects'] = projection.typingEffects;
      value['wordbook_packs'] = projection.wordbookPacks;
      value['symbol_set_packs'] = projection.symbolSetPacks;
      // 背单词卡片的喇叭：设备没有文本转语音能力时不画。
      value['read_aloud'] = HarmonyReadAloud.available();
      value['touch_toolbar_tools'] = projection.touchToolbarTools;
      value['touch_key_sound_packs'] = projection.touchKeySoundPacks;
      value['touch_plugin_packs'] = projection.touchPluginPacks;
      value['candidate_skin_editor'] = projection.candidateSkinEditor;
      value['touch_one_handed_side_panel'] = projection.oneHandedSidePanel;
      value['touch_keyboard_split'] = projection.splitKeyboard;
      value['harmony_shared_sandbox'] = this.sharedSandboxAvailable;
      if (release !== null) {
        value['os_version'] = release;
      }
      // Cantonese, Zhuyin and Stroke are offered only with their dictionary installed, as the desktop app's `drop_uninstalled_language_schemes` does; the keyboard leaves such a scheme out of its picker by the same rule.
      const schemes: Object | undefined = value['input_schemes'];
      if (Array.isArray(schemes)) {
        value['input_schemes'] = (schemes as string[]).filter((scheme: string): boolean =>
          this.languageDictionaryInstalled(scheme));
      }
      return JSON.stringify(reply);
    } catch (error) {
      // A capability document this cannot read is forwarded untouched: the page has a fallback for
      // a missing version and none for a reply that has stopped being one.
      KeyboardLog.error(`host capabilities unreadable: ${describeError(error)}`);
      return document;
    }
  }

  dictionary(action: string): string {
    if (this.context === undefined) {
      return JSON.stringify({ ok: false, error: 'settings context unavailable' });
    }
    // 词库更新的 `automatic` 和 `check` 会去下载，没有隐私政策的授权就不发。
    if (!HarmonyPrivacyConsent.networkAllowed() && PrivacyConsentPolicy.dictionaryActionNeedsConsent(action)) {
      return PrivacyConsentPolicy.refusal();
    }
    const response: string = KeyboardSession.shared.dictionary(action, `${this.context.filesDir}/engine`,
      this.stateDirectory);
    // 页面切走后仍观察任务完成，把完整代次发布给键盘；镜像只随安装选择变化。
    try {
      const request: Record<string, Object> = JSON.parse(action) as Record<string, Object>;
      if (request['operation'] === 'updates') {
        this.publishShared();
        const reply: Record<string, Object> = JSON.parse(response) as Record<string, Object>;
        if (reply['ok'] === true) {
          const status: Record<string, Object> = reply['value'] as Record<string, Object>;
          if ((status['phase'] === 'checking' || status['phase'] === 'downloading') && this.dictionaryUpdateTimer === -1) {
            this.dictionaryUpdateTimer = setTimeout((): void => {
              this.dictionaryUpdateTimer = -1;
              this.dictionary('{"operation":"updates","command":"status"}');
            }, 5000);
          }
        }
      }
    } catch (_) {}
    return response;
  }

  /** 词库页的命名词库：建、删、启停、加词、导入、装社区词库。导入可达 16 MiB，所以走 startRequest 在原生工作线程上做。 */
  private dictionaryCollections(action: string): Promise<string> {
    if (this.context === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'collections_io' }));
    }
    return KeyboardSession.shared.dictionaryCollections(action, `${this.context.filesDir}/engine`,
      this.stateDirectory);
  }

  /** 账号请求留在原生层，WebView 只看到安全的 JSON DTO。 */
  account(action: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'account_unavailable' }));
    }
    return this.accountBridge.request(action);
  }

  /** 昵称纯计算可以同步调用，不访问账号或存储。 */
  accountName(request: string): string {
    return client.accountName(request);
  }

  /** 云剪贴板面板的动作，经 Rust 的端到端加密驱动；按偏好走水杉账号、自建中继或 WebDAV 网盘。 */
  cloudClipboard(action: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'account_unavailable' }));
    }
    return this.accountBridge.cloudClipboard(action);
  }

  /** 「剪贴板」页 WebDAV 设置的「测试连接」，草稿是 `{url, username, password}`。 */
  cloudClipboardWebDavProbe(draft: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'relay_unavailable' }));
    }
    return this.accountBridge.cloudClipboardWebDavProbe(draft);
  }

  /** 「词库」页的「词库同步」，页面交来的是 `msime_client_dictionary_sync` 的 `action`；除 `status` 外都要隐私政策的授权（`startRequest` 开头已经拦过）。 */
  dictionarySync(action: string): Promise<string> {
    if (this.context === undefined || this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'dictionary_sync_storage' }));
    }
    return this.accountBridge.dictionarySync(action, `${this.context.filesDir}/engine`, this.stateDirectory);
  }

  cloudDictionary(action: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'account_unavailable' }));
    }
    return this.accountBridge.cloudDictionary(action, this.stateDirectory);
  }

  /** Validate and queue one explicitly selected cloud entry for the keyboard's idle worker. */
  cloudDictionaryDownload(entryDocument: string): string {
    if (this.context === undefined) {
      return JSON.stringify({ ok: false, error: 'settings context unavailable' });
    }
    try {
      const entry: CloudDownloadEntry = JSON.parse(entryDocument) as CloudDownloadEntry;
      const kind: string = entry.kind;
      const code: string = entry.code;
      const word: string = entry.word;
      const weight: number = entry.weight;
      // `wubi98` 落到本地同名的 98 五笔个人词库，与 `wubi` 分开。
      if (typeof kind !== 'string' || !['pinyin', 'wubi', 'wubi98', 'quick', 'english'].includes(kind)
        || typeof code !== 'string' || code.length === 0 || code.length > 256
        || typeof word !== 'string' || word.length === 0 || word.length > 1024
        || typeof weight !== 'number' || !Number.isInteger(weight) || weight < 0) {
        return JSON.stringify({ ok: false, error: 'account_invalid' });
      }
      const replacement: LocalDictionaryEntry = {
        kind: kind === 'quick' ? 'quick_phrase' : kind,
        key: code,
        value: word,
        weight: weight
      };
      this.cloudDownloadRequest += 1;
      const request: LocalDictionaryEditRequest = {
        operation: 'queue_edit',
        previous: null,
        replacement: replacement,
        request_id: `cloud-download-${this.cloudDownloadRequest}`
      };
      return KeyboardSession.shared.dictionary(JSON.stringify(request), `${this.context.filesDir}/engine`,
        this.stateDirectory);
    } catch {
      return JSON.stringify({ ok: false, error: 'account_invalid' });
    }
  }

  /**
   * The settings sync, which reads and writes the same preference document this page edits.
   *
   * The state directory is the host's, not the page's: the page names an operation and an account,
   * and where the document lives is not something a WebView should be able to say.
   */
  settingsSync(action: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'account_unavailable' }));
    }
    return this.accountBridge.preferencesSync(action, this.stateDirectory)
      .then((reply: string): string => {
        // Applying is the one operation that rewrites the document under the page's feet. The
        // page's own copy is still the one it read on the way in, so every control keeps showing
        // the old value and the next save is refused as a conflict. Hand it the new document here
        // rather than waiting for the window to be brought forward again — that listener exists for
        // changes the keyboard makes, and a change this window made should not need an app switch
        // to become visible.
        this.refreshPreferencesIfChanged();
        return reply;
      });
  }

  cloudDictionarySnapshot(action: string): Promise<string> {
    if (this.accountBridge === undefined || this.context === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'snapshot_unavailable' }));
    }
    return this.accountBridge.snapshot(action, `${this.context.filesDir}/engine`, this.stateDirectory);
  }

  /**
   * 背单词卡片的喇叭：页面交来卡片上的词头，先按共享层的规则（`msime_client_read_aloud_text`）核对它是英文，再用系统语音读出。读出来时 `{ok:true}`，读不出来时 `{ok:false,error:给用户看的那句话}`，页面原样显示。
   */
  private async readAloud(payload: string): Promise<string> {
    const word: string | null = ReadAloudPolicy.requestText(payload);
    const spoken: string | null = word === null ? null
      : ReadAloudPolicy.textFromReply(client.readAloudText(ReadAloudPolicy.textRequest(word, '')));
    if (spoken === null) {
      return JSON.stringify({ ok: false, error: ReadAloudPolicy.FAILED });
    }
    const notice: string | null = await HarmonyReadAloud.speak(spoken);
    return notice === null ? JSON.stringify({ ok: true, value: null }) : JSON.stringify({ ok: false, error: notice });
  }

  /**
   * 背单词: forward one action to the shared review session.
   *
   * The numbered bridge keeps the potentially multi-megabyte import and its file work off the
   * ArkTS thread. The day is resolved here because this process knows the device's timezone.
   *
   * The day is resolved here rather than in the page. This process is the one that knows the
   * device's timezone, and the shared layer never derives a day from one.
   */
  private async vocabularyReview(actionDocument: string): Promise<string> {
    // 原始词表仍限 8 MiB；这里检查的是带 JSON 转义的动作，与原生桥和 C 入口的请求预算一致。
    if (actionDocument.length === 0 || actionDocument.length > 17 * 1024 * 1024) {
      return JSON.stringify({ ok: false, error: 'invalid vocabulary review request' });
    }
    try {
      const action: VocabularyAction = JSON.parse(actionDocument) as VocabularyAction;
      if (!['load', 'answer', 'set_settings', 'import', 'remove', 'remove_personal_word', 'reset'].includes(action.operation)) {
        return JSON.stringify({ ok: false, error: 'invalid vocabulary review request' });
      }
      if (this.context === undefined) {
        return JSON.stringify({ ok: false, error: 'vocabulary review failed' });
      }
      const now: Date = new Date();
      const month: string = `${now.getMonth() + 1}`.padStart(2, '0');
      const date: string = `${now.getDate()}`.padStart(2, '0');
      const request = VocabularyReviewRequest.build(
        this.stateDirectory,
        this.context.resourceDir,
        `${now.getFullYear()}-${month}-${date}`,
        action,
        SettingsFormFactorCapabilities.resolve(deviceInfo.deviceType).wordbookPacks,
      );
      return await client.vocabularyReview(JSON.stringify(request));
    } catch (error) {
      KeyboardLog.error(`vocabulary review failed: ${describeError(error)}`);
      return JSON.stringify({ ok: false, error: 'vocabulary review failed' });
    }
  }

  private async statisticsAuthenticationSettings(payload: string): Promise<string> {
    if (this.context === undefined) throw new Error('认证宿主未就绪');
    const action: TypingStatisticsAction = JSON.parse(payload) as TypingStatisticsAction;
    if (action.operation === 'save' && typeof action.enabled === 'boolean') {
      await this.statisticsAuthentication.setEnabled(this.context, action.enabled);
    } else if (action.operation !== 'load') {
      throw new Error('无效的认证设置');
    }
    return JSON.stringify({ ok: true, value: this.statisticsAuthentication.enabled(this.context) });
  }

  /**
   * 「防止截屏」的读和写（#10194）。写入按 `ScreenCaptureProtectionPolicy.decide`：第一次打开直接保存，打开过以后先过系统认证，设备没有系统认证时拒绝。保存后当场更新本窗口的隐私模式，并立即发布给键盘（键盘下一次获得焦点时给面板设上）。拒绝时回答错误码，页面据此提示并让开关回到原值：`screen_capture_authentication_unavailable` 没有可用的系统认证，`screen_capture_authentication_failed` 认证取消或失败，`screen_capture_busy` 上一次切换还没完成。
   */
  private async screenCaptureProtectionSettings(payload: string): Promise<string> {
    const context: common.UIAbilityContext | undefined = this.context;
    if (context === undefined) throw new Error('认证宿主未就绪');
    const action: ScreenCaptureProtectionAction = JSON.parse(payload) as ScreenCaptureProtectionAction;
    const current: ScreenCaptureProtectionState = ScreenCaptureProtection.load(this.stateDirectory);
    if (action.operation === 'load') {
      return JSON.stringify({ ok: true, value: current.enabled } as FlagReply);
    }
    const next: boolean | undefined = action.enabled;
    if (action.operation !== 'save' || typeof next !== 'boolean') throw new Error('无效的防止截屏设置');
    if (this.screenCaptureBusy) return JSON.stringify({ ok: false, error: 'screen_capture_busy' });
    const decision: ScreenCaptureProtectionDecision = ScreenCaptureProtectionPolicy.decide(current, next,
      ScreenCaptureAuthentication.available());
    if (decision === ScreenCaptureProtectionDecision.UNCHANGED) {
      return JSON.stringify({ ok: true, value: current.enabled } as FlagReply);
    }
    if (decision === ScreenCaptureProtectionDecision.UNAVAILABLE) {
      return JSON.stringify({ ok: false, error: 'screen_capture_authentication_unavailable' });
    }
    this.screenCaptureBusy = true;
    try {
      if (decision === ScreenCaptureProtectionDecision.AUTHENTICATE
        && !(await this.screenCaptureAuthentication.authenticate(next))) {
        return JSON.stringify({ ok: false, error: 'screen_capture_authentication_failed' });
      }
      const saved: ScreenCaptureProtectionState = ScreenCaptureProtectionPolicy.after(current, next);
      if (!ScreenCaptureProtection.save(this.stateDirectory, saved)) throw new Error('防止截屏没有保存');
      KeyboardLog.info(`screen capture protection ${saved.enabled ? 'on' : 'off'}`);
      ScreenCaptureProtection.protectLastWindow(context, saved.enabled);
      // 文件已经写好、窗口也已改过，发布失败不能让页面以为没保存而把开关退回旧值；只记日志，窗口退到后台时的 `publishShared` 会再发布一次。
      try {
        this.publishShared();
      } catch (error) {
        KeyboardLog.error(`screen capture protection not published to the keyboard: ${describeError(error)}`);
      }
      return JSON.stringify({ ok: true, value: saved.enabled } as FlagReply);
    } finally {
      this.screenCaptureBusy = false;
    }
  }

  typingStatistics(actionDocument: string): string {
    if (actionDocument.length === 0 || actionDocument.length > 4096) {
      return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
    }
    try {
      const action: TypingStatisticsAction = JSON.parse(actionDocument) as TypingStatisticsAction;
      if (action.operation === 'cancel_authentication') {
        this.statisticsAuthentication.cancel();
        return JSON.stringify({ ok: true, value: true });
      }
      if (!['load', 'set_enabled', 'set_retention', 'reset'].includes(action.operation)
        || (action.operation === 'set_enabled' && typeof action.enabled !== 'boolean')
        || (action.operation === 'set_retention'
          && !['forever', '30d', '90d', '180d', '365d'].includes(action.retention ?? ''))) {
        return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
      }
      // The store counts the retention window back from the caller's local day and refuses a set_retention without one. This process is the one that knows the device's timezone, as for vocabularyReview above.
      if (action.operation === 'set_retention') {
        action.day = TypingStatisticsPolicy.day(new Date());
      }
      const raw: string = client.typingStatistics(JSON.stringify({
        directory: this.stateDirectory, action: action
      }));
      const reply: TypingStatisticsReply = JSON.parse(raw) as TypingStatisticsReply;
      if (reply.ok !== true) return raw;
      // 开关、保留期和清空都要告诉键盘：统计由键盘记录，设置应用这里只是一份副本。
      if (action.operation !== 'load' && this.sharedMirror !== null) {
        SharedStateSync.writeStatisticsSwitch(this.stateDirectory, action.operation === 'reset');
        this.schedulePublish();
      }
      if (reply.value === undefined || reply.value === null) {
        return JSON.stringify({ ok: false, error: 'invalid typing statistics response' });
      }
      let lastWrittenMs: number | null = null;
      let available: boolean = false;
      const file: string = `${this.stateDirectory}/typing-statistics.json`;
      try {
        if (fs.accessSync(file)) {
          available = true;
          // `Stat.mtime` 是秒，页面要毫秒；直接传秒会被当成 1970 年 1 月的某一刻。
          lastWrittenMs = fs.statSync(file).mtime * 1000;
        }
      } catch {
        available = false;
      }
      const status: TypingStatisticsStatusValue = {
        statistics: reply.value,
        availability: available ? 'ready' : 'neverWritten',
        lastWrittenMs: lastWrittenMs
      };
      const result: TypingStatisticsStatusReply = { ok: true, value: status };
      return JSON.stringify(result);
    } catch {
      return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
    }
  }

  /**
   * 存储真正要干活的统计操作 `summary` 和 `record_skin`，经 `startRequest` 到达并在原生工作线程上运行：两者都要取键盘进程同样会取的文件锁，而 `summary` 从整个文档推导每个时间窗口，所以都不能阻塞这个线程（Android 也在工作线程上运行它们）。
   */
  private async typingStatisticsOnWorker(actionDocument: string): Promise<string> {
    if (actionDocument.length === 0 || actionDocument.length > 4096) {
      return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
    }
    let action: TypingStatisticsAction;
    try {
      action = JSON.parse(actionDocument) as TypingStatisticsAction;
    } catch {
      return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
    }
    if (action.operation === 'summary') {
      return await this.typingStatisticsSummary(action);
    }
    if (action.operation === 'record_skin') {
      return await this.recordSkinTried(action);
    }
    return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
  }

  /**
   * 「统计」页的派生数据：概览、习惯、按键和徽章，新达成的徽章由存储记下。
   *
   * 回复是存储自己的 `{ok,value}`，而不是 load 回复的状态包装，因为 summary 是计算出来的，不是从页面要显示其时间的文件里读出来的。日期在这里补上，与 `set_retention` 一样，因为这个进程知道设备的时区。
   */
  private async typingStatisticsSummary(action: TypingStatisticsAction): Promise<string> {
    const words: number | undefined = action.user_words;
    if (words !== undefined && (typeof words !== 'number' || !Number.isSafeInteger(words) || words < 0)) {
      return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
    }
    const summary: TypingStatisticsSummaryAction = {
      operation: 'summary', day: TypingStatisticsPolicy.day(new Date())
    };
    if (words !== undefined) {
      summary.user_words = words;
    }
    const request: TypingStatisticsSummaryRequest = { directory: this.stateDirectory, action: summary };
    return await client.typingStatisticsAsync(JSON.stringify(request));
  }

  /**
   * 打字统计「上传到云端」。构建请求、十分钟的节流、设备标识、上次上传的时间和登录账号都在 Rust 里（`msime_client_account` 的 `typing_upload_*`），同一进程里的打开、关闭和上传也由 Rust 串行。
   */
  private typingUpload(document: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'account_unavailable' }));
    }
    return this.accountBridge.typingUpload(document);
  }

  /** 记下应用了某个皮肤，供「换装达人」徽章使用。皮肤 id 的形式由存储决定；这里只限制大小。 */
  private async recordSkinTried(action: TypingStatisticsAction): Promise<string> {
    const id: string = action.id ?? '';
    if (typeof id !== 'string' || id.length === 0 || id.length > 128) {
      return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
    }
    const skin: TypingStatisticsSkinAction = { operation: 'record_skin', id: id };
    const request: TypingStatisticsSkinRequest = { directory: this.stateDirectory, action: skin };
    return await client.typingStatisticsAsync(JSON.stringify(request));
  }

  async aiModels(requestDocument: string): Promise<string> {
    let request: AiModelsRequest;
    try {
      request = JSON.parse(requestDocument) as AiModelsRequest;
      if (typeof request.endpoint !== 'string' || typeof request.token !== 'string'
        || typeof request.provider !== 'string') {
        return JSON.stringify({ ok: false, error: 'ai_models_invalid' });
      }
    } catch {
      return JSON.stringify({ ok: false, error: 'ai_models_invalid' });
    }
    const endpoint: string | null = AiModelCatalogPolicy.modelsUrl(request.endpoint,
      (value: string): boolean => AiEndpoint.allowed(value));
    const token: string = request.token.trim();
    if (endpoint === null || token.length === 0 || utf8Length(token) > 16 * 1024
      || TextPolicy.hasControl(token)) {
      return JSON.stringify({ ok: false, error: 'ai_invalid' });
    }
    const anthropic: boolean = AiModelCatalogPolicy.isAnthropic(request.provider, endpoint);
    const models: string[] = [];
    const cursors: string[] = [];
    let cursor: string = '';
    for (let pageNumber: number = 0; pageNumber < 10; pageNumber += 1) {
      const httpRequest = http.createHttp();
      installNoRedirectGuard(httpRequest);
      try {
        const headers: AiModelHeaders = anthropic
          ? {
            Accept: 'application/json', 'x-api-key': token,
            'anthropic-version': '2023-06-01'
          }
          : { Accept: 'application/json', Authorization: `Bearer ${token}` };
        const response: http.HttpResponse = await httpRequest.request(
          AiModelCatalogPolicy.pageUrl(endpoint, anthropic, cursor), {
            method: http.RequestMethod.GET,
            header: headers,
            connectTimeout: 5000,
            readTimeout: 30000
          });
        if (response.responseCode < 200 || response.responseCode >= 300
          || typeof response.result !== 'string' || utf8Length(response.result) > 1024 * 1024) {
          return JSON.stringify({ ok: false, error: 'ai_models_unavailable' });
        }
        const document: AiCatalogPage = JSON.parse(response.result) as AiCatalogPage;
        if (!AiModelCatalogPolicy.append(models, document)) {
          return JSON.stringify({ ok: false, error: 'ai_models_invalid' });
        }
        const next: string | null = AiModelCatalogPolicy.nextCursor(document, anthropic, cursors);
        if (next === null) return JSON.stringify({ ok: false, error: 'ai_models_invalid' });
        if (next.length === 0) {
          if (models.length === 0) return JSON.stringify({ ok: false, error: 'ai_models_invalid' });
          models.sort();
          return JSON.stringify({ ok: true, value: models });
        }
        cursors.push(next);
        cursor = next;
      } catch (error) {
        KeyboardLog.error(`AI model list unavailable: ${describeError(error)}`);
        return JSON.stringify({ ok: false, error: 'ai_models_unavailable' });
      } finally {
        httpRequest.destroy();
      }
    }
    return JSON.stringify({ ok: false, error: 'ai_models_invalid' });
  }

  async aiTest(requestDocument: string): Promise<string> {
    let request: AiTestRequest;
    try {
      request = JSON.parse(requestDocument) as AiTestRequest;
      if (typeof request.endpoint !== 'string' || typeof request.model !== 'string'
        || typeof request.prompt !== 'string' || typeof request.token !== 'string'
        || typeof request.text !== 'string') {
        return JSON.stringify({ ok: false, error: 'ai_invalid' });
      }
    } catch {
      return JSON.stringify({ ok: false, error: 'ai_invalid' });
    }
    const endpoint: string | null = this.validAiEndpoint(request.endpoint);
    const token: string = request.token.trim();
    const model: string = request.model.trim();
    if (endpoint === null || token.length === 0 || utf8Length(token) > 16 * 1024
      || TextPolicy.hasControl(token) || model.length === 0 || utf8Length(model) > 256
      || TextPolicy.hasControl(model) || !TextPolicy.validMultiline(request.prompt, 16 * 1024, true)
      || !TextPolicy.validMultiline(request.text, 16 * 1024, false)) {
      return JSON.stringify({ ok: false, error: 'ai_invalid' });
    }
    const body: AiTestBody = {
      model: model,
      stream: false,
      temperature: 0.2,
      messages: [
        { role: 'system', content: request.prompt },
        { role: 'user', content: request.text }
      ]
    };
    // ArkTS has no object spread; headers() hands back a fresh record, so adding to it is safe.
    const header: Record<string, string> = AiAuthenticationPolicy.headers(endpoint, token);
    header['Content-Type'] = 'application/json';
    const httpRequest = http.createHttp();
    installNoRedirectGuard(httpRequest);
    try {
      const response: http.HttpResponse = await httpRequest.request(endpoint, {
        method: http.RequestMethod.POST,
        header: header,
        connectTimeout: 5000,
        readTimeout: 15000,
        extraData: JSON.stringify(body)
      });
      if (response.responseCode < 200 || response.responseCode >= 300
        || typeof response.result !== 'string' || utf8Length(response.result) > 1024 * 1024) {
        return JSON.stringify({ ok: false, error: 'ai_test_unavailable' });
      }
      const document: AiCompletionDocument = JSON.parse(response.result) as AiCompletionDocument;
      const content: string | undefined = document.choices?.[0]?.message?.content;
      if (content === undefined || !TextPolicy.validMultiline(content, 16 * 1024, false)) {
        return JSON.stringify({ ok: false, error: 'ai_test_invalid' });
      }
      return JSON.stringify({ ok: true, value: content.trim() });
    } catch (error) {
      KeyboardLog.error(`AI test unavailable: ${describeError(error)}`);
      return JSON.stringify({ ok: false, error: 'ai_test_unavailable' });
    } finally {
      httpRequest.destroy();
    }
  }

  async testApiCredential(requestDocument: string): Promise<string> {
    let request: ApiCredentialRequest;
    try {
      request = JSON.parse(requestDocument) as ApiCredentialRequest;
    } catch {
      return this.credentialReply(false, '配置格式无效。');
    }
    if (request.config === undefined || typeof request.service !== 'string') {
      return this.credentialReply(false, 'Harmony 当前只支持 AI 配置测试。');
    }
    if (request.service === 'ai.assistant') {
      const endpoint: string = request.config.endpoint ?? '';
      const token: string = request.config.token ?? '';
      const provider: string = request.config.provider ?? 'custom';
      const modelReply: string = await this.aiModels(JSON.stringify({ endpoint, token, provider }));
      const result: AiReply<string[]> = JSON.parse(modelReply) as AiReply<string[]>;
      if (result.ok !== true) {
        return this.credentialReply(false, '无法连接 AI 服务，请检查接口地址和 Token。');
      }
      return this.credentialReply(true, `连接成功，可用模型 ${result.value.length} 个。`);
    }
    if (request.service === 'translation.custom') {
      return this.translationCredential(request.config);
    }
    if (request.service === 'translation.tencent') {
      return this.tencentCredential(request.config);
    }
    if (request.service === 'translation.niutrans') {
      return this.niutransCredential(request.config);
    }
    if (request.service === 'voice.polish') {
      return this.chatCredential(request.config);
    }
    if (request.service === 'voice.asr') {
      if ((request.config.provider ?? '').trim() === 'doubao') {
        return this.doubaoCredential(request.config);
      }
      return this.asrCredential(request.config);
    }
    return this.credentialReply(false, 'Harmony 不支持此配置测试类型。');
  }

  private credentialReply(ok: boolean, message: string): string {
    const value: ApiCredentialResult = { ok, message };
    const reply: ApiCredentialReply = { ok: true, value };
    return JSON.stringify(reply);
  }

  private async translationCredential(config: ApiCredentialConfig): Promise<string> {
    const endpoint: string | null = this.validCredentialEndpoint(config.endpoint ?? '', true);
    const apiKey: string = (config.api_key ?? '').trim();
    if (endpoint === null || (apiKey.length > 0 && !this.validCredentialSecret(apiKey, 4096))) {
      return this.credentialReply(false, '请填写有效的翻译接口地址和 API Key。');
    }
    // 与键盘同一个共享描述器：DeepL 官方地址换成 DeepL 的鉴权和正文，其余按 DeepLX 兼容格式，测试与实际翻译不会各说各的。
    const probe: CustomTranslationProbeRequest = CustomTranslationProbePolicy.request(endpoint, apiKey);
    let descriptor: CustomTranslationProbeDescriptor | null = null;
    try {
      const reply: CustomTranslationProbeDescriptorReply = JSON.parse(
        client.customTranslationHttpRequest(JSON.stringify(probe))) as CustomTranslationProbeDescriptorReply;
      descriptor = reply.ok === true && reply.value !== undefined ? reply.value : null;
    } catch {
      descriptor = null;
    }
    if (descriptor === null || this.validCredentialEndpoint(descriptor.url, true) === null) {
      return this.credentialReply(false, '请填写有效的翻译接口地址和 API Key。');
    }
    const deepl: boolean = CustomTranslationProbePolicy.isDeepL(descriptor);
    const headers: Record<string, string> = CustomTranslationProbePolicy.headers(descriptor);
    const request = http.createHttp();
    installNoRedirectGuard(request);
    try {
      const response: http.HttpResponse = await request.request(descriptor.url, {
        method: http.RequestMethod.POST,
        header: headers,
        connectTimeout: 5000,
        readTimeout: 15000,
        extraData: JSON.stringify(descriptor.body)
      });
      if (!CustomTranslationProbePolicy.acceptsResponse(response.responseCode, response.result)) {
        return this.credentialReply(false, CustomTranslationProbePolicy.failureMessage(deepl, response.responseCode));
      }
      const parsed: CustomTranslationProbeGlossReply = JSON.parse(
        client.parseCustomTranslationResponse(response.result as string)) as CustomTranslationProbeGlossReply;
      const translated: string | null = parsed.ok === true && typeof parsed.value === 'string' ? parsed.value : null;
      if (translated !== null) return this.credentialReply(true, CustomTranslationProbePolicy.SUCCESS);
      return this.credentialReply(false, CustomTranslationProbePolicy.failureMessage(deepl, response.responseCode));
    } catch (error) {
      KeyboardLog.error(`translation credential unavailable: ${describeError(error)}`);
      return this.credentialReply(false, '连接失败，请检查翻译接口地址、API Key 和网络。');
    } finally {
      request.destroy();
    }
  }

  private async tencentCredential(config: ApiCredentialConfig): Promise<string> {
    const secretId: string = (config.secret_id ?? '').trim();
    const secretKey: string = (config.secret_key ?? '').trim();
    const region: string = (config.region ?? '').trim();
    if (!this.validCredentialSecret(secretId, 4096) || !this.validCredentialSecret(secretKey, 4096)
      || region.length > 64 || TextPolicy.characters(region).some((character: string): boolean => {
        return !(/[A-Za-z0-9-]/.test(character));
      })) {
      return this.credentialReply(false, '请填写有效的腾讯云 SecretId、SecretKey 和地域。');
    }
    const milliseconds: number = Date.now();
    const timestamp: number = Math.floor(milliseconds / 1000);
    const date: string = new Date(milliseconds).toISOString().substring(0, 10);
    const body: TencentTranslationBody = {
      Source: 'zh', Target: 'en', ProjectId: 0, SourceTextList: ['测试']
    };
    const bodyDocument: string = JSON.stringify(body);
    const authorization: string = await tencentAuthorization(
      secretId, secretKey, timestamp, date, bodyDocument);
    const request = http.createHttp();
    installNoRedirectGuard(request);
    try {
      const response: http.HttpResponse = await request.request('https://tmt.tencentcloudapi.com', {
        method: http.RequestMethod.POST,
        header: {
          Accept: 'application/json', 'Content-Type': 'application/json; charset=utf-8',
          Host: 'tmt.tencentcloudapi.com', 'X-TC-Action': 'TextTranslateBatch',
          'X-TC-Timestamp': String(timestamp), 'X-TC-Version': '2018-03-21',
          'X-TC-Region': region.length > 0 ? region : 'ap-guangzhou', Authorization: authorization
        },
        connectTimeout: 5000,
        readTimeout: 15000,
        extraData: bodyDocument
      });
      if (response.responseCode < 200 || response.responseCode >= 300
        || typeof response.result !== 'string' || utf8Length(response.result) > 256 * 1024) {
        return this.credentialReply(false, '测试失败：腾讯云未返回有效译文。');
      }
      const document: TencentTranslationDocument = JSON.parse(response.result) as TencentTranslationDocument;
      const translated: string[] | undefined = document.Response?.TargetTextList;
      if (document.Response?.Error !== undefined || translated === undefined
        || translated.length !== 1 || translated[0].trim().length === 0) {
        return this.credentialReply(false, '测试失败：腾讯云未返回有效译文。');
      }
      return this.credentialReply(true, '连接成功，翻译服务配置有效。');
    } catch (error) {
      KeyboardLog.error(`Tencent translation credential unavailable: ${describeError(error)}`);
      return this.credentialReply(false, '连接失败，请检查腾讯云凭据、地域和网络。');
    } finally {
      request.destroy();
    }
  }

  private async niutransCredential(config: ApiCredentialConfig): Promise<string> {
    const appId: string = (config.app_id ?? '').trim();
    const apiKey: string = (config.apikey ?? '').trim();
    if (!this.validCredentialSecret(appId, 4096) || !this.validCredentialSecret(apiKey, 4096)) {
      return this.credentialReply(false, '请填写有效的小牛翻译 App ID 和 API Key。');
    }
    const timestamp: string = String(Date.now());
    const authStr: string = await niutransAuth(appId, apiKey, timestamp, '测试');
    const form: string = [
      `from=${encodeURIComponent('zh')}`,
      `to=${encodeURIComponent('en')}`,
      `appId=${encodeURIComponent(appId)}`,
      `timestamp=${encodeURIComponent(timestamp)}`,
      `srcText=${encodeURIComponent('测试')}`,
      `authStr=${encodeURIComponent(authStr)}`
    ].join('&');
    const request = http.createHttp();
    installNoRedirectGuard(request);
    try {
      const response: http.HttpResponse = await request.request(
        'https://api.niutrans.com/v2/text/translate', {
          method: http.RequestMethod.POST,
          header: { Accept: 'application/json', 'Content-Type': 'application/x-www-form-urlencoded; charset=utf-8' },
          connectTimeout: 5000,
          readTimeout: 15000,
          extraData: form
        });
      if (response.responseCode < 200 || response.responseCode >= 300
        || typeof response.result !== 'string' || utf8Length(response.result) > 256 * 1024) {
        return this.credentialReply(false, '测试失败：小牛翻译未返回有效译文。');
      }
      const document: NiuTransDocument = JSON.parse(response.result) as NiuTransDocument;
      if (document.errorCode !== undefined || document.errorMsg !== undefined
        || document.tgtText === undefined || document.tgtText.trim().length === 0) {
        return this.credentialReply(false, '测试失败：小牛翻译未返回有效译文。');
      }
      return this.credentialReply(true, '连接成功，翻译服务配置有效。');
    } catch (error) {
      KeyboardLog.error(`NiuTrans credential unavailable: ${describeError(error)}`);
      return this.credentialReply(false, '连接失败，请检查小牛翻译凭据和网络。');
    } finally {
      request.destroy();
    }
  }

  private async chatCredential(config: ApiCredentialConfig): Promise<string> {
    const endpoint: string | null = this.validCredentialEndpoint(config.endpoint ?? '', false);
    const token: string = (config.token ?? '').trim();
    const model: string = (config.model ?? '').trim();
    const provider: string = (config.provider ?? '').trim();
    if (endpoint === null || !this.validCredentialSecret(token, 8192) || model.length === 0
      || utf8Length(model) > 256 || TextPolicy.hasControl(model)
      || !['openai', 'siliconflow', 'deepseek', 'groq'].includes(provider)) {
      return this.credentialReply(false, '请填写有效的 HTTPS 接口地址、API Key 和模型名。');
    }
    const body: ChatCredentialBody = {
      model: model,
      stream: false,
      max_tokens: 1,
      messages: [{ role: 'user', content: 'Reply OK' }]
    };
    if (provider === 'deepseek') body.thinking = { type: 'disabled' };
    if (provider === 'siliconflow') body.enable_thinking = false;
    const request = http.createHttp();
    installNoRedirectGuard(request);
    try {
      const response: http.HttpResponse = await request.request(endpoint, {
        method: http.RequestMethod.POST,
        header: { Accept: 'application/json', 'Content-Type': 'application/json',
          Authorization: `Bearer ${token}` },
        connectTimeout: 5000,
        readTimeout: 15000,
        extraData: JSON.stringify(body)
      });
      if (response.responseCode >= 200 && response.responseCode < 300) {
        return this.credentialReply(true, '连接成功，API Key 和模型配置有效。');
      }
      return this.credentialReply(false,
        CredentialStatusPolicy.rejected(response.responseCode, '服务拒绝请求，请检查接口地址和模型配置'));
    } catch (error) {
      KeyboardLog.error(`chat credential unavailable: ${describeError(error)}`);
      return this.credentialReply(false, '连接失败，请检查接口地址、模型、API Key 和网络。');
    } finally {
      request.destroy();
    }
  }

  private async asrCredential(config: ApiCredentialConfig): Promise<string> {
    const provider: string = (config.provider ?? '').trim();
    const endpoint: string | null = this.validCredentialEndpoint(config.endpoint ?? '', false);
    const token: string = (config.token ?? '').trim();
    const model: string = (config.model ?? '').trim();
    const format: string = HttpAsrConfigurationPolicy.requestFormat(provider);
    if (format.length === 0 || endpoint === null
      || !this.validCredentialSecret(token, 8192) || model.length === 0
      || utf8Length(model) > 256 || TextPolicy.hasControl(model)) {
      return this.credentialReply(false, '请填写有效的 HTTPS 转写接口地址、API Key 和模型名。');
    }
    const boundary: string = '----MSIMEHarmonyCredentialProbe';
    // 按请求格式拼一秒静音：chat_audio 是 JSON，其余是 multipart。
    const chat: boolean = format === HTTP_ASR_CHAT_AUDIO;
    const body: ArrayBuffer | string = chat
      ? HttpAsrConfigurationPolicy.chatAudioBody(model,
        new util.Base64Helper().encodeToStringSync(this.silentWav()))
      : this.silentMultipart(boundary, model);
    const request = http.createHttp();
    installNoRedirectGuard(request);
    try {
      const response: http.HttpResponse = await request.request(endpoint, {
        method: http.RequestMethod.POST,
        header: { Accept: 'application/json', Authorization: `Bearer ${token}`,
          'Content-Type': chat ? 'application/json' : `multipart/form-data; boundary=${boundary}` },
        connectTimeout: 5000,
        readTimeout: 15000,
        extraData: body
      });
      if (response.responseCode >= 200 && response.responseCode < 300) {
        return this.credentialReply(true, '连接成功，API Key 和模型配置有效。');
      }
      return this.credentialReply(false,
        CredentialStatusPolicy.rejected(response.responseCode, '转写测试未成功，请检查接口地址、模型和服务配置'));
    } catch (error) {
      KeyboardLog.error(`ASR credential unavailable: ${describeError(error)}`);
      return this.credentialReply(false, '连接失败，请检查转写接口、模型、API Key 和网络。');
    } finally {
      request.destroy();
    }
  }

  private async doubaoCredential(config: ApiCredentialConfig): Promise<string> {
    const endpoint: string | null = this.validDoubaoEndpoint(config.endpoint ?? '');
    const authMode: string = (config.auth_mode ?? '').trim();
    const appId: string = (config.app_id ?? '').trim();
    const token: string = (config.token ?? '').trim();
    const resourceId: string = (config.resource_id ?? '').trim();
    const boostingTableId: string = (config.doubao_boosting_table_id ?? '').trim();
    if (endpoint === null || !['api_key', 'legacy'].includes(authMode)
      || !this.validCredentialSecret(token, 8192)
      || (authMode === 'legacy' && !this.validCredentialSecret(appId, 8192))
      || !this.validCredentialSecret(resourceId, 256)
      || utf8Length(boostingTableId) > 256 || TextPolicy.hasControl(boostingTableId)) {
      return this.credentialReply(false, '请填写有效的 WSS 接口、资源 ID 及对应鉴权方式的凭据。');
    }
    const requestId: string = util.generateRandomUUID(false);
    const connectionOptions: webSocket.WebSocketRequestOptions = authMode === 'legacy'
      ? { header: { 'x-api-resource-id': resourceId, 'x-api-request-id': requestId,
          'x-api-app-key': appId, 'x-api-access-key': token } }
      : { header: { 'x-api-resource-id': resourceId, 'x-api-request-id': requestId,
          'x-api-key': token } };
    const socket: webSocket.WebSocket = webSocket.createWebSocket();
    let settled: boolean = false;
    let connectTimer: number | undefined = undefined;
    let totalTimer: number | undefined = undefined;
    const finish = (ok: boolean): void => {
      if (settled) return;
      settled = true;
      if (connectTimer !== undefined) clearTimeout(connectTimer);
      if (totalTimer !== undefined) clearTimeout(totalTimer);
      socket.close().catch((): void => undefined);
    };
    const result: Promise<boolean> = new Promise((resolve: (value: boolean) => void): void => {
      const complete = (ok: boolean): void => {
        if (settled) return;
        finish(ok);
        resolve(ok);
      };
      socket.on('error', (): void => complete(false));
      socket.on('close', (_error: Object, _result: webSocket.CloseResult): void => complete(false));
      socket.on('message', (_error: Object, value: string | ArrayBuffer): void => {
        if (settled || typeof value === 'string') {
          if (typeof value === 'string') complete(false);
          return;
        }
        const decoded: DoubaoFrameResult | null = client.doubaoDecodeFrame(value);
        if (decoded === null || utf8Length(decoded.payload) > 1024 * 1024) {
          complete(false);
          return;
        }
        try {
          const document: DoubaoResponseDocument = JSON.parse(decoded.payload) as DoubaoResponseDocument;
          if (document === null || Array.isArray(document)
            || document.error !== undefined && document.error !== null) {
            complete(false);
            return;
          }
          if (document.code !== undefined && document.code !== 0) {
            complete(false);
            return;
          }
          if (document.payload_msg !== undefined && document.payload_msg.code !== undefined
            && document.payload_msg.code !== 0) {
            complete(false);
            return;
          }
          if (decoded.last) complete(true);
        } catch {
          complete(false);
        }
      });
      connectTimer = setTimeout((): void => complete(false), 5000);
      totalTimer = setTimeout((): void => complete(false), 15000);
      socket.connect(endpoint, connectionOptions).then((connected: boolean): void => {
        if (settled) return;
        if (!connected) {
          complete(false);
          return;
        }
        if (connectTimer !== undefined) clearTimeout(connectTimer);
        const start: DoubaoStartRequest = {
          user: { uid: 'metasequoia-ime' },
          audio: { format: 'pcm', codec: 'raw', rate: 16000, bits: 16, channel: 1 },
          request: {
            model_name: 'bigmodel',
            enable_itn: config.doubao_enable_itn !== false,
            enable_punc: config.doubao_enable_punc !== false,
            enable_ddc: config.doubao_enable_ddc === true,
            show_utterances: false,
            result_type: 'full'
          }
        };
        if (boostingTableId.length > 0) {
          start.request.corpus = { boosting_table_id: boostingTableId } as DoubaoCorpus;
        }
        const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
        const startPayload: ArrayBuffer = encoder.encodeInto(JSON.stringify(start)).buffer as ArrayBuffer;
        const audioPayload: ArrayBuffer = new Uint8Array(32000).buffer as ArrayBuffer;
        const startFrame: ArrayBuffer = client.doubaoEncodeFrame(1, 1, 1, startPayload);
        const finalFrame: ArrayBuffer = client.doubaoEncodeFrame(2, 3, -2, audioPayload);
        socket.send(startFrame).then((first: boolean): Promise<boolean> => {
          return first ? socket.send(finalFrame) : Promise.resolve(false);
        }).then((second: boolean): void => {
          if (!second) complete(false);
        }).catch((): void => complete(false));
      }).catch((): void => complete(false));
    });
    const ok: boolean = await result;
    return this.credentialReply(ok, ok
      ? '连接成功，豆包语音识别凭据有效。'
      : '豆包测试未成功或超时，请检查凭据、资源权限、接口地址和网络。');
  }

  private silentMultipart(boundary: string, model: string): ArrayBuffer {
    const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
    const wav: Uint8Array = this.silentWav();
    const prefix: Uint8Array = encoder.encodeInto(`--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="credential-test.wav"\r\nContent-Type: audio/wav\r\n\r\n`);
    const modelPart: Uint8Array = encoder.encodeInto(`\r\n--${boundary}\r\nContent-Disposition: form-data; name="model"\r\n\r\n${model}\r\n--${boundary}--\r\n`);
    const output: ArrayBuffer = new ArrayBuffer(prefix.length + wav.length + modelPart.length);
    const bytes: Uint8Array = new Uint8Array(output);
    bytes.set(prefix, 0);
    bytes.set(wav, prefix.length);
    bytes.set(modelPart, prefix.length + wav.length);
    return output;
  }

  /** 一秒 16 kHz 单声道 16 位静音 WAV，凭据测试用。 */
  private silentWav(): Uint8Array {
    const wav: Uint8Array = new Uint8Array(44 + 32000);
    this.writeAscii(wav, 0, 'RIFF');
    this.writeU32(wav, 4, 32036);
    this.writeAscii(wav, 8, 'WAVEfmt ');
    this.writeU32(wav, 16, 16);
    this.writeU16(wav, 20, 1);
    this.writeU16(wav, 22, 1);
    this.writeU32(wav, 24, 16000);
    this.writeU32(wav, 28, 32000);
    this.writeU16(wav, 32, 2);
    this.writeU16(wav, 34, 16);
    this.writeAscii(wav, 36, 'data');
    this.writeU32(wav, 40, 32000);
    return wav;
  }

  private writeAscii(bytes: Uint8Array, offset: number, value: string): void {
    const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
    bytes.set(encoder.encodeInto(value), offset);
  }

  private writeU16(bytes: Uint8Array, offset: number, value: number): void {
    bytes[offset] = value & 0xff;
    bytes[offset + 1] = (value >>> 8) & 0xff;
  }

  private writeU32(bytes: Uint8Array, offset: number, value: number): void {
    bytes[offset] = value & 0xff;
    bytes[offset + 1] = (value >>> 8) & 0xff;
    bytes[offset + 2] = (value >>> 16) & 0xff;
    bytes[offset + 3] = (value >>> 24) & 0xff;
  }

  private validCredentialEndpoint(value: string, allowHttp: boolean): string | null {
    const endpoint: string = value.trim();
    if (!TextPolicy.validSecureAuthority(endpoint, allowHttp)) return null;
    return endpoint;
  }

  private validDoubaoEndpoint(value: string): string | null {
    const endpoint: string = value.trim();
    if (!TextPolicy.validAuthority(endpoint, ['wss://'])) {
      return null;
    }
    return endpoint;
  }

  private validCredentialSecret(value: string, maxBytes: number): boolean {
    return value.length > 0 && utf8Length(value) <= maxBytes && !TextPolicy.hasControl(value)
      && !value.startsWith('<') && !value.startsWith('FAKESECRET_')
      && !TextPolicy.characters(value).every((character: string): boolean => character === '*');
  }

  /** AI 接口地址：https 不限主机，http 只能指向本机或局域网（Rust 的规则，见 AiEndpoint）。 */
  private validAiEndpoint(value: string): string | null {
    const endpoint: string = value.trim();
    if (!AiEndpoint.allowed(endpoint)) return null;
    return endpoint;
  }

  appVersion(): string {
    try {
      const info: bundleManager.BundleInfo = bundleManager.getBundleInfoForSelfSync(
        bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT);
      return info.versionName;
    } catch (error) {
      KeyboardLog.error(`version unreadable: ${describeError(error)}`);
      return '';
    }
  }

  /** 发布日期 `YYYY-MM-DD`，构建时由 entry/hvigorfile.ts 写进 `BuildProfile`；构建时无从得知则为空串。 */
  appReleaseDate(): string {
    return BuildProfile.RELEASE_DATE;
  }

  /**
   * 隐私政策的授权：`{operation:"status"}` 回复 `{ok,value:{granted,decided}}`；`{operation:"review"}` 另外打开授权弹窗（「隐私」页的「查看并同意」），选择的结果之后经 `msimeHarmonyPrivacyConsentChanged` 送达页面。
   */
  privacyConsent(request: string): string {
    let operation: string = '';
    try {
      const action: PrivacyConsentAction = JSON.parse(request) as PrivacyConsentAction;
      operation = typeof action.operation === 'string' ? action.operation : '';
    } catch {
      operation = '';
    }
    if (operation === 'review') {
      this.onPrivacyReview?.();
    } else if (operation !== 'status') {
      return JSON.stringify({ ok: false, error: 'privacy_consent_invalid' });
    }
    const reply: PrivacyConsentReply = { ok: true, value: HarmonyPrivacyConsent.state() };
    return JSON.stringify(reply);
  }

  /**
   * 弹窗里的选择：记在当前版本上，立即发布给键盘（键盘下一次获得焦点就按它联网或不联网），再告诉页面重建联网的入口。不同意时清空本机排队待发的使用统计：用户没同意上传，就不该留着等下次。
   */
  decidePrivacy(granted: boolean): void {
    HarmonyPrivacyConsent.decide(this.stateDirectory, granted, Date.now());
    if (!granted) {
      HarmonyTelemetry.reportingTurnedOff(this.stateDirectory);
    }
    this.publishShared();
    this.webview?.runJavaScript(
      `globalThis.msimeHarmonyPrivacyConsentChanged && globalThis.msimeHarmonyPrivacyConsentChanged(${granted})`)
      .catch((error: Object): void => {
        KeyboardLog.error(`privacy consent change not delivered: ${describeError(error)}`);
      });
  }

  openExternalUrl(url: string): void {
    // Only the two schemes a settings page has any business opening; anything else is not a link the
    // page should be able to make this application follow.
    if (!TextPolicy.validExternalWebUrl(url)) {
      KeyboardLog.error('refused to open a non-web link');
      return;
    }
    const want: Want = { action: 'ohos.want.action.viewData', entities: ['entity.system.browsable'], uri: url };
    if (this.context !== undefined) {
      this.context.startAbility(want).catch((error: Object): void => {
        KeyboardLog.error(`link would not open: ${describeError(error)}`);
        // 设备上没有能打开网页的应用时 startAbility 失败。页面和首次启动弹窗都把打开交给这里，同步拿不到结果，所以由这里把地址告诉用户，免得登录面板和隐私弹窗里的协议链接点了没反应。
        this.uiContext?.getPromptAction().showToast({ message: TextPolicy.unopenedLinkNotice(url), duration: 4000 });
      });
    }
  }

  copyText(text: string): void {
    const data: pasteboard.PasteData = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text);
    pasteboard.getSystemPasteboard().setData(data).catch((error: Object): void => {
      KeyboardLog.error(`copy failed: ${describeError(error)}`);
    });
  }

  /**
   * Key sound and haptics, which the keyboard stores beside its own state rather than in the shared
   * preferences: how a key feels belongs to the device being typed on, not to the account.
   *
   * The settings page is a second writer of the same file the keyboard reads. 键盘每次聚焦编辑框时重读它（`KeyboardSession.refreshFeedback`），所以这里的改动在下一次弹出键盘时生效，不必等输入法进程重启。
   */
  keyboardFeedback(request: string): string {
    this.schedulePublish();
    let action: KeyboardFeedbackAction;
    try {
      action = JSON.parse(request) as KeyboardFeedbackAction;
    } catch (error) {
      return JSON.stringify({ ok: false, value: KeyboardFeedbackBridge.toShared(
        KeyboardSession.feedbackIn(this.stateDirectory)), error: 'invalid feedback action' } as KeyboardFeedbackReply);
    }
    if (action.operation === 'preview') {
      this.previewFeedback(action.strength ?? 'medium');
      return JSON.stringify({ ok: true, value: KeyboardFeedbackBridge.toShared(
        KeyboardSession.feedbackIn(this.stateDirectory)), error: '' } as KeyboardFeedbackReply);
    }
    if (action.operation === 'preview_sound') {
      this.previewKeySound(action.pack, action.volume);
      return JSON.stringify({ ok: true, value: KeyboardFeedbackBridge.toShared(
        KeyboardSession.feedbackIn(this.stateDirectory)), error: '' } as KeyboardFeedbackReply);
    }
    if (action.operation === 'save') {
      // 页面没有发送的字段（旧页面没有手写设置）保留已存的值，而不是重置。
      const settings: FeedbackSettings = KeyboardFeedbackBridge.fromShared(action.settings ?? null,
        KeyboardSession.feedbackIn(this.stateDirectory));
      if (!KeyboardSession.saveFeedbackIn(this.stateDirectory, settings)) {
        return JSON.stringify({ ok: false, value: KeyboardFeedbackBridge.toShared(
          KeyboardSession.feedbackIn(this.stateDirectory)), error: '按键反馈未能保存' } as KeyboardFeedbackReply);
      }
      return JSON.stringify({ ok: true, value: KeyboardFeedbackBridge.toShared(settings),
        error: '' } as KeyboardFeedbackReply);
    }
    return JSON.stringify({ ok: true, value: KeyboardFeedbackBridge.toShared(
      KeyboardSession.feedbackIn(this.stateDirectory)), error: '' } as KeyboardFeedbackReply);
  }

  /** 用 `pack` 的普通键样本试听一声。包 id 由 `msime_client_key_sound_pack` 校验；页面给的音量不在 0–100 时按默认的 50。 */
  private previewKeySound(pack: string | undefined, volume: number | undefined): void {
    const context: common.UIAbilityContext | undefined = this.context;
    if (context === undefined || typeof pack !== 'string' || pack.length === 0) {
      return;
    }
    const gainVolume: number = typeof volume === 'number' && volume >= 0 && volume <= 100 ? volume : 50;
    this.keySoundPreview.play(this.soundPacks(), this.stateDirectory, `${context.cacheDir}/key-sound-preview`,
      pack, gainVolume);
  }

  /** One buzz at the strength being previewed, exactly as a key press at that strength would feel (`KeyboardSession.vibrate`). A device without a vibrator simply does nothing. */
  private previewFeedback(strength: string): void {
    try {
      KeyboardSession.vibrate(KeyboardFeedbackBridge.strength(strength)).catch((error: Object): void => {
        KeyboardLog.error(`feedback preview unavailable: ${describeError(error)}`);
      });
    } catch (error) {
      KeyboardLog.error(`feedback preview unavailable: ${describeError(error)}`);
    }
  }

  private safeDirectory(directory: string): boolean {
    try {
      const stat: fs.Stat = fs.lstatSync(directory);
      return !stat.isSymbolicLink() && stat.isDirectory();
    } catch {
      return false;
    }
  }

  private symbolicLink(path: string): boolean {
    try {
      return fs.lstatSync(path).isSymbolicLink();
    } catch {
      return false;
    }
  }

  /**
   * The installed font families, for the candidate font inputs.
   *
   * The shared page validates and sorts what it gets; the only thing done here is to refuse names
   * carrying a control character, which have no business in a preference file or a family list.
   * Deduplication and ordering are the page's, so every host presents the same list.
   */
  listFontFamilies(): string {
    try {
      if (this.uiContext === undefined) throw new Error('settings UI context unavailable');
      const families: string[] = this.uiContext.getFont().getSystemFontList().filter((name: string): boolean =>
        name.length > 0 && name.length <= 128 && !/[\u0000-\u001f\u007f]/.test(name));
      return JSON.stringify({ ok: true, value: families, error: '' } as FontFamilyReply);
    } catch (error) {
      KeyboardLog.error(`system fonts unavailable: ${describeError(error)}`);
      return JSON.stringify({ ok: false, value: [], error: '无法读取系统字体列表' } as FontFamilyReply);
    }
  }

  /**
   * The microphones the page can offer, in the shared settings shape.
   *
   * Wrapped like every other reply so an unreadable routing manager arrives as a refusal the picker
   * can report rather than an empty list that reads as "this device has no microphone".
   */
  listVoiceCaptureDevices(): string {
    const devices: VoiceCaptureDevice[] | null = HarmonyVoiceCaptureDevices.list();
    if (devices === null) {
      return JSON.stringify({ ok: false, value: [], error: '无法读取录音设备列表' } as VoiceCaptureDeviceReply);
    }
    return JSON.stringify({ ok: true, value: devices, error: '' } as VoiceCaptureDeviceReply);
  }

  /**
   * Whether setup still has a step left in it.
   *
   * Asked once when the window opens, so the page can show the welcome flow instead of a settings
   * page for a keyboard that cannot yet type. Both queries are allowed to fail: a refusal answers
   * "unknown", which the policy reads as "show settings" rather than sending a long-time user back
   * to a welcome screen because one system call did not answer.
   */
  /**
   * Whether setup still has a step left, from a value computed before the page asked.
   *
   * Synchronous on purpose. A bridge method that returns a Promise is delivered to the page through
   * a channel that does not settle on this platform build — registered in the sync list the page
   * receives something it cannot await, and registered in the async list, by either the attribute or
   * the controller, the promise never resolves and the page waits on its startup screen forever.
   * Both were tried on a device. So the answer is prepared while the window is being created and
   * handed over as a plain value.
   *
   * Not yet computed reads as "no onboarding", which is the safe direction: the settings page is
   * reachable from the welcome flow and the welcome flow is not reachable from settings.
   */
  onboardingStatus(): string {
    return JSON.stringify({
      ok: true, value: this.onboardingRequired && !this.onboardingDone, ready: this.onboardingReady, error: ''
    } as OnboardingReply);
  }

  /**
   * 「设置」首页卡片和欢迎流程各步骤显示的两项设置状态，与 `onboardingStatus` 来自同一次读取。
   *
   * 同样是同步的，理由也相同：首次读取完成前 `ready` 为 false，页面会等待，而不是把某一步画成未完成。
   */
  setupStatus(): string {
    const reply: SetupStatusReply = { ok: true, ready: this.onboardingReady, value: this.setup };
    return JSON.stringify(reply);
  }

  /** 资源使用实际暂存结果，系统能力使用同一次系统查询；引导离开记录独立持久化。 */
  oobeStatus(): string {
    const preferences: RevisionReply = JSON.parse(client.loadPreferences(this.stateDirectory)) as RevisionReply;
    const value: OobeStatusValue = {
      resourcesReady: this.resourcesReady,
      systemEnabled: this.setup.enabled,
      systemCurrent: this.setup.current,
      preferencesSaved: preferences.ok === true,
      onboardingSeen: this.onboardingDone
    };
    const reply: OobeStatusReply = { ok: true, value };
    return JSON.stringify(reply);
  }

  /** 设置页提供显式重试，不把暂存失败当成资源已经就绪。 */
  retryOobeResources(): string {
    if (this.context !== undefined) {
      this.resourcesReady = StagedResources.stage(`${this.context.resourceDir}/engine`,
        `${this.context.filesDir}/engine`, `${this.context.filesDir}/engine.ready`);
    }
    const reply: FlagReply = { ok: this.resourcesReady === true, value: this.resourcesReady === true };
    return JSON.stringify(reply);
  }

  /** 页面请求现在重读一次设置状态。从键盘的 🌐 菜单切换输入法不会触发 `imeChange`，页面的「设为默认」引导打开期间靠它轮询；读到变化时照常推给页面。 */
  refreshSetupStatus(): void {
    this.refreshSetupState();
  }

  /** 页面的欢迎流程已结束（完成或跳过），通知卡片可以回来。 */
  onboardingFinished(): void {
    this.onboardingDone = true;
    if (!this.writeStateFile(ONBOARDING_SEEN_FILE, '{"seen":true}')) {
      KeyboardLog.error('onboarding flag not saved');
    }
    this.onNoticeHold?.(false);
  }

  /** 页面从设置里重放欢迎流程，流程显示期间重新压住通知卡片；流程结束时页面调用 `onboardingFinished` 让卡片回来。`onboardingDone` 保持不变：它只在首次读到设置状态时被读取，而能重放时首次读取早已完成。 */
  onboardingStarted(): void {
    this.onNoticeHold?.(true);
  }

  /** 重新读取设置状态，并把任何失败记进日志；读取本身已经为每次被拒的系统调用记了日志。 */
  refreshSetupState(): void {
    this.readSetupState().catch((error: Object): void => {
      KeyboardLog.error(`setup state not read: ${describeError(error)}`);
    });
  }

  /** 开始监听系统当前键盘的变化；设置页面出现时调用一次，页面离开时由 `unwatchSetup` 撤销。 */
  watchSetup(): void {
    try {
      inputMethod.getSetting().on('imeChange', this.imeChanged);
    } catch (error) {
      // 激活时的重新读取仍能捕捉到变化，只是更晚：要等窗口下次回到前台。
      KeyboardLog.error(`input method changes not observable: ${describeError(error)}`);
    }
  }

  unwatchSetup(): void {
    try {
      inputMethod.getSetting().off('imeChange', this.imeChanged);
    } catch (error) {
      KeyboardLog.error(`input method change listener not removed: ${describeError(error)}`);
    }
  }

  /** 设置文档已加载，宿主到页面的通知现在有监听方了。 */
  pageLoaded(): void {
    this.pageReady = true;
  }

  /**
   * 读取本键盘是否已启用、是否为当前键盘，并在任一项变化时告知页面。
   *
   * 在窗口打开时、每次回到前台时、系统当前键盘改变时以及键盘选择器返回后运行，因为每个时刻用户都可能刚在系统界面里完成了一步设置。首次结果还决定窗口是否以欢迎流程打开，以及通知卡片是否保持不出现在流程上。
   */
  async readSetupState(): Promise<void> {
    this.setupReads += 1;
    const read: number = this.setupReads;
    let enabled: ImeEnabledState | null = null;
    try {
      const state: inputMethod.EnabledState =
        await inputMethod.getSetting().getInputMethodState();
      enabled = state as number as ImeEnabledState;
    } catch (error) {
      KeyboardLog.error(`input method state unavailable: ${describeError(error)}`);
    }
    let current: string = '';
    try {
      current = inputMethod.getCurrentInputMethod().name;
    } catch (error) {
      KeyboardLog.error(`current input method unavailable: ${describeError(error)}`);
    }
    // Read rather than written down: a bundle name in two places is a bundle name that can disagree
    // with itself, and the copy here would be the one nobody updates.
    let own: string = '';
    try {
      own = bundleManager.getBundleInfoForSelfSync(
        bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT).name;
    } catch (error) {
      KeyboardLog.error(`own bundle name unavailable: ${describeError(error)}`);
    }
    if (read !== this.setupReads) {
      // 这次读取等待系统期间又开始了一次更晚的读取，那次的结果更新。
      return;
    }
    const state: OnboardingState = {
      enabled: enabled, currentBundle: current, ownBundle: own
    };
    const description: string = OnboardingStatePolicy.describe(state);
    if (description !== this.setupDescription) {
      // 只在变化时记日志，而不是每次激活都记，这样日志反映的是设置状态变化的时刻。
      KeyboardLog.info(`onboarding: ${description}`);
      this.setupDescription = description;
    }
    const setup: SetupState = OnboardingStatePolicy.setup(state);
    const changed: boolean = setup.enabled !== this.setup.enabled || setup.current !== this.setup.current;
    const first: boolean = !this.onboardingReady;
    this.setup = setup;
    this.onboardingRequired = OnboardingStatePolicy.required(state);
    this.onboardingReady = true;
    if (first) {
      this.onNoticeHold?.(this.onboardingRequired && !this.onboardingDone);
    }
    // 首次结果也推送，不只是之后的变化：页面会轮询几秒然后停止，比这慢的首次读取否则永远到不了页面。文档加载前 `runPageScript` 会跳过它，由页面在启动时自己读取结果。
    if (first || changed) {
      this.runPageScript(
        `globalThis.msimeHarmonySetupChanged && globalThis.msimeHarmonySetupChanged(${JSON.stringify(setup)})`,
        'setup change');
    }
  }

  /**
   * 调用页面挂在 `globalThis` 上的某个监听函数。文档加载前跳过：页面在启动时会自己读取同样的状态，而对尚未就绪的页面发起的调用本来也会丢失。
   */
  private runPageScript(script: string, what: string): void {
    if (this.webview === undefined || !this.pageReady) {
      return;
    }
    this.webview.runJavaScript(script).catch((error: Object): void => {
      KeyboardLog.error(`${what} not delivered: ${describeError(error)}`);
    });
  }

  /**
   * The system's keyboard picker.
   *
   * Enabling an input method and selecting it are two different screens, and this is the second.
   * Windows has neither — there is nothing to pick — which is why this sits behind the shared
   * welcome flow's action rather than anywhere in the settings page itself.
   */
  /**
   * Opens the system keyboard picker and answers at once.
   *
   * The page has nothing to do with the result — the picker is a system screen and the user's
   * choice shows up in the onboarding state next time it is read — and an answer that never
   * arrives would leave the welcome flow's button spinning. A refusal is logged rather than
   * returned for the same reason.
   */
  showInputMethodPicker(): string {
    // 选择器是覆盖在本窗口上的对话框，所以窗口可能始终不离开前台，激活时的重新读取不会运行。在选择器返回时以及系统键盘变化事件上重新读取，「设为默认」这一步才能往前推进。
    try {
      if (this.onInputMethodPicker === undefined) throw new Error('settings picker unavailable');
      this.onInputMethodPicker();
    } catch (error) {
      KeyboardLog.error(`keyboard picker would not open: ${describeError(error)}`);
      return JSON.stringify({ ok: false, value: false, ready: true, error: '无法打开输入法选择器' } as OnboardingReply);
    }
    return JSON.stringify({ ok: true, value: true, ready: true, error: '' } as OnboardingReply);
  }

  /**
   * 应用主题（水杉四季和四个季节），为本宿主自己的页面着色，并给键盘的「跟随系统」皮肤加色调。
   *
   * `load` 和 `save` 读写状态目录里的设备本地选择，键盘也读这个文件；`resolve` 把存储的选择转成本月的颜色以及页面的浅色或深色模式；`catalog` 列出可选项。两个原生回复原样返回，页面读到的结构与每个宿主一致。
   */
  appTheme(request: string): string {
    this.schedulePublish();
    const refused: string = JSON.stringify({ ok: false, error: 'invalid app theme request' } as AppThemeReply);
    if (request.length === 0 || request.length > 4096) {
      return refused;
    }
    let action: AppThemeAction;
    try {
      action = JSON.parse(request) as AppThemeAction;
    } catch {
      return refused;
    }
    if (action === null || typeof action !== 'object') {
      return refused;
    }
    if (action.operation === 'load') {
      const value: AppThemeIdValue = { id: this.storedAppTheme() };
      return JSON.stringify({ ok: true, value: value } as AppThemeReply);
    }
    if (action.operation === 'save') {
      const id: string = action.id ?? '';
      if (typeof id !== 'string' || !AppThemeStore.isId(id)) {
        return refused;
      }
      if (!this.writeStateFile(AppThemeStore.FILE_NAME, AppThemeStore.serialize(id))) {
        return JSON.stringify({ ok: false, error: 'app_theme_storage' } as AppThemeReply);
      }
      const value: AppThemeIdValue = { id: id };
      return JSON.stringify({ ok: true, value: value } as AppThemeReply);
    }
    if (action.operation === 'resolve') {
      if (typeof action.dark !== 'boolean') {
        return refused;
      }
      // 月份取设备本地的：水杉四季按用户的日历换季，而不是按 UTC。
      const resolve: AppThemeResolveRequest = {
        app_theme: this.storedAppTheme(), month: new Date().getMonth() + 1, dark: action.dark
      };
      return client.resolveAppTheme(JSON.stringify(resolve));
    }
    if (action.operation === 'catalog') {
      return client.appThemeCatalog();
    }
    return refused;
  }

  /** 请页面重新解析应用主题，窗口在后台期间可能已经跨月。 */
  notifyAppThemeChanged(): void {
    this.runPageScript('globalThis.msimeHarmonyAppThemeChanged && globalThis.msimeHarmonyAppThemeChanged()',
      'app theme change');
  }

  /** 存储的应用主题 id；文件缺失或无法读取时为默认值，规则同 `AppThemeStore.parse`。 */
  private storedAppTheme(): string {
    return AppThemeStore.parse(this.readStateFile(AppThemeStore.FILE_NAME, AppThemeStore.MAX_BYTES));
  }

  /**
   * 给状态栏和导航栏着色以匹配页面，这只有宿主能做到。
   *
   * 页面发来背景色以及是否为深色，让栏上的图标和文字在其上保持可读。被拒时记日志，其他方面忽略：系统栏保留系统的颜色，只是外观差异，不是值得报告给页面的失败。
   */
  setSystemBars(document: string): void {
    let bars: SystemBarsRequest;
    try {
      bars = JSON.parse(document) as SystemBarsRequest;
    } catch {
      KeyboardLog.error('system bar colours refused: unreadable request');
      return;
    }
    const colour: RegExp = /^#[0-9A-Fa-f]{6}$/;
    if (bars === null || typeof bars !== 'object' || typeof bars.background !== 'string'
      || typeof bars.navigationBar !== 'string' || typeof bars.dark !== 'boolean'
      || !colour.test(bars.background) || !colour.test(bars.navigationBar)) {
      KeyboardLog.error('system bar colours refused: invalid request');
      return;
    }
    this.onPageBackground?.(bars.background);
    if (this.launchCovered) {
      this.heldSystemBars = document;
      return;
    }
    if (this.context === undefined) {
      return;
    }
    const content: string = bars.dark ? '#FFFFFF' : '#000000';
    const properties: window.SystemBarProperties = {
      statusBarColor: bars.background,
      statusBarContentColor: content,
      navigationBarColor: bars.navigationBar,
      navigationBarContentColor: content
    };
    window.getLastWindow(this.context).then((main: window.Window): Promise<void> => {
      // 页面避开了底部手势条那一块，露出来的是窗口自己的底色（默认白色），导航栏颜色画不到那里：开屏时底部一条白，设置页标签栏下面也一条白。窗口底色取导航栏的颜色，那一块就和标签栏或开屏连成一片。
      try {
        main.setWindowBackgroundColor(bars.navigationBar);
      } catch (error) {
        KeyboardLog.error(`window background not applied: ${describeError(error)}`);
      }
      return main.setWindowSystemBarProperties(properties);
    }).catch((error: Object): void => {
      KeyboardLog.error(`system bar colours not applied: ${describeError(error)}`);
    });
  }

  /** 欢迎开屏是否已在本设备上显示过，让它只播放一次，而不是每次首次运行流程都播放。 */
  splashSeen(): string {
    const reply: FlagReply = { ok: true, value: this.readStateFile(SPLASH_SEEN_FILE, 64) !== null };
    return JSON.stringify(reply);
  }

  /**
   * 记下开屏已播放。存放在宿主文件里而不是页面的 `localStorage`，因为从应用资源加载的页面不保证保留它。
   */
  markSplashSeen(): void {
    if (!this.writeStateFile(SPLASH_SEEN_FILE, '{"seen":true}')) {
      KeyboardLog.error('splash flag not saved');
    }
  }

  /** 页面的第一帧已经上屏（或宿主等够了时限），撤掉盖在网页上的启动画面，并补上盖着期间页面要的系统栏颜色。 */
  launchPainted(): void {
    if (!this.launchCovered) {
      return;
    }
    this.launchCovered = false;
    this.onLaunchPainted?.();
    const held: string | undefined = this.heldSystemBars;
    this.heldSystemBars = undefined;
    if (held !== undefined) {
      this.setSystemBars(held);
    }
  }

  /** 设置页面加载完成前键盘请求打开的页面和它的子目标，只交出一次：第二次读取返回 null。 */
  pendingPage(): string {
    const link: SettingsLinkRequest | null = EntryAbility.pendingPage;
    EntryAbility.pendingPage = null;
    const reply: PendingPageReply = {
      ok: true,
      value: link === null ? null : link.page,
      target: link === null ? null : link.target,
    };
    return JSON.stringify(reply);
  }

  /**
   * 在本窗口已显示时打开键盘请求的页面。
   *
   * 页面还在加载、尚无监听函数时，请求保留为待处理，让页面启动时的读取能找到它，而不是丢失。子目标作为第二个参数交给 `msimeHarmonyOpenPage`，没有时只传页面。
   */
  openPage(link: SettingsLinkRequest): void {
    if (this.webview === undefined || !this.pageReady) {
      EntryAbility.pendingPage = link;
      return;
    }
    const args: string = link.target === null
      ? JSON.stringify(link.page)
      : `${JSON.stringify(link.page)}, ${JSON.stringify(link.target)}`;
    const script: string = `globalThis.msimeHarmonyOpenPage ? (globalThis.msimeHarmonyOpenPage(${args}), true) : false`;
    this.webview.runJavaScript(script).then((result: string): void => {
      if (result !== 'true') {
        EntryAbility.pendingPage = link;
      }
    }).catch((error: Object): void => {
      EntryAbility.pendingPage = link;
      KeyboardLog.error(`linked page not opened: ${describeError(error)}`);
    });
  }

  /**
   * 状态目录里的一个设备本地小文件；缺失、过大或不是普通文件时为 null。占据其位置的链接会被拒绝而不是跟随，与键盘对待它自己在那里的文件一样。
   *
   * 打开后在文件描述符上再核对一次类型、大小和 inode，并从这个描述符读取，这样检查和读取之间被换掉的文件不会被读进来，与键盘的 `readPrivateText` 相同。
   */
  private readStateFile(name: string, maximumBytes: number): string | null {
    const path: string = `${this.stateDirectory}/${name}`;
    let handle: fs.File | null = null;
    try {
      if (!this.safeDirectory(this.stateDirectory)) {
        return null;
      }
      const expected: fs.Stat = fs.lstatSync(path);
      if (expected.isSymbolicLink() || !expected.isFile() || expected.size > maximumBytes) {
        return null;
      }
      handle = fs.openSync(path, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > maximumBytes || stat.ino !== expected.ino) {
        return null;
      }
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) {
        return null;
      }
      return util.TextDecoder.create('utf-8').decodeToString(new Uint8Array(buffer, 0, count));
    } catch {
      // 从未写过的文件是常见情况，不是错误。
      return null;
    } finally {
      if (handle !== null) {
        NativeCleanup.close(handle);
      }
    }
  }

  /**
   * 替换状态目录里的一个设备本地小文件。先写在旁边再重命名覆盖，这样在自己进程里读同一批文件的键盘永远不会看到写了一半的文档。
   */
  private writeStateFile(name: string, contents: string): boolean {
    const path: string = `${this.stateDirectory}/${name}`;
    const staging: string = `${path}.${util.generateRandomUUID(false)}.tmp`;
    try {
      if (!fs.accessSync(this.stateDirectory)) {
        fs.mkdirSync(this.stateDirectory, true);
      }
      if (!this.safeDirectory(this.stateDirectory) || this.symbolicLink(path)) {
        return false;
      }
      const file: fs.File = fs.openSync(staging, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        const written: number = fs.writeSync(file.fd, contents);
        if (!utf8WriteComplete(contents, written)) throw new Error('state_write');
      } finally {
        fs.closeSync(file);
      }
      fs.renameSync(staging, path);
      return true;
    } catch (error) {
      KeyboardLog.error(`${name} not saved: ${describeError(error)}`);
      try {
        if (fs.accessSync(staging)) {
          fs.unlinkSync(staging);
        }
      } catch {
        // 要紧的是上面的失败；残留的暂存文件用的名字没有任何地方会读。
      }
      return false;
    }
  }


  /** Where the user turns the keyboard on, which is a system screen rather than one of ours. */
  openSystemKeyboardSettings(): void {
    const want: Want = {
      bundleName: 'com.huawei.hmos.settings',
      abilityName: 'com.huawei.hmos.settings.MainAbility',
      uri: 'input_method_settings'
    };
    if (this.context !== undefined) {
      this.context.startAbility(want).catch((error: Object): void => {
        KeyboardLog.error(`keyboard settings would not open: ${describeError(error)}`);
      });
    }
  }
}
