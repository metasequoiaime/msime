import web_webview from '@ohos.web.webview';
import common from '@ohos.app.ability.common';
import client, { DoubaoFrameResult } from 'libmsimeclient.so';
import pasteboard from '@ohos.pasteboard';
import Want from '@ohos.app.ability.Want';
import bundleManager from '@ohos.bundle.bundleManager';
import font from '@ohos.font';
import vibrator from '@ohos.vibrator';
import deviceInfo from '@ohos.deviceInfo';
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
import { HarmonyAiSkins } from '../account/HarmonyAiSkins';
import { utf8Length } from '../keyboard/Utf8';
import { TextPolicy } from '../keyboard/TextPolicy';
import { TypingStatisticsPolicy } from '../keyboard/TypingStatisticsPolicy';
import { niutransAuth, tencentAuthorization } from '../keyboard/CredentialCrypto';
import { HarmonyVoiceCaptureDevices } from '../keyboard/input/HarmonyVoiceCaptureDevices';
import { KeyboardFeedbackBridge, MobileKeyboardFeedback } from '../keyboard/KeyboardFeedbackBridge';
import inputMethod from '@ohos.inputMethod';
import { PreferenceRevisionPolicy } from '../keyboard/input/PreferenceRevisionPolicy';
import { PreferencesErrorCode } from '../keyboard/settings/PreferencesErrorCode';
import picker from '@ohos.file.picker';
import { SkinImportPolicy } from '../keyboard/skin/SkinImportPolicy';
import {
  PickedEntryKind, PluginFolderScan, PluginImportPolicy, PluginImportRefusal
} from '../keyboard/settings/PluginImportPolicy';
import EntryAbility from '../entryability/EntryAbility';
import { ImeEnabledState, OnboardingState, OnboardingStatePolicy }
  from '../keyboard/input/OnboardingStatePolicy';
import { FeedbackSettings } from '../keyboard/KeyboardFeedback';
import { VoiceCaptureDevice } from '../keyboard/input/VoiceCaptureDevicePolicy';
import { OsVersionPolicy } from '../keyboard/input/OsVersionPolicy';
import {
  AiCatalogPage, AiModelCatalogPolicy
} from '../keyboard/settings/AiModelCatalogPolicy';
import { SettingsFormFactorCapabilities, SettingsFormFactorProjection }
  from '../keyboard/settings/SettingsFormFactorCapabilities';
import { LocalVoiceModelAction, LocalVoiceModelPolicy } from '../keyboard/settings/LocalVoiceModelPolicy';
import { AiAuthenticationPolicy } from '../keyboard/settings/AiAuthenticationPolicy';
import { HarmonyTelemetry } from '../telemetry/Telemetry';
import { TelemetryPolicy } from '../telemetry/TelemetryPolicy';
import { NoticeItem, NoticePolicy } from '../notices/NoticePolicy';

/** `{root}` for the list, `{root, id, mirror}` for an install, `{root, id}` for a removal, `{id}` for a cancel. */
interface LocalVoiceModelRequest {
  root?: string;
  id?: string;
  mirror?: string;
}

/** One key-feedback operation from the page: load, save or preview. */
interface KeyboardFeedbackAction {
  operation: string;
  settings?: MobileKeyboardFeedback;
  strength?: string;
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

interface OnboardingReply {
  ok: boolean;
  value: boolean;
  /** False until the answer has been computed; the page waits rather than acting on a default. */
  ready: boolean;
  error: string;
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

/** `{directory, platform, channel}` for `client.notices`; `{directory, id}` for `client.noticeDismiss`. */
interface NoticesRequest {
  directory: string;
  platform?: string;
  channel?: string;
  id?: string;
}

/** One AI skin request from the page: start a run, or stop the one under that id. */
interface AiSkinAction {
  operation: string;
  request_id?: string;
  prompt?: string;
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
  resource_operation: string;
  id?: string;
  item?: Object;
}

/** One community skin request from the page; only download and finish_trial are handled here. */
interface CommunitySkinAction {
  community_operation: string;
  id: string;
  name?: string;
  keep?: boolean;
}

interface CommunityDownloadValue {
  design?: Object;
}

interface CommunityDownloadReply {
  ok: boolean;
  value?: CommunityDownloadValue;
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
}

/** One 插件 page request; only the operation is read here, the rest travels to `msime_client_plugins` as the page wrote it. */
interface PluginPageAction {
  operation: string;
}

/** The page's import request: which picker to show. */
interface PluginImportRequest {
  source: string;
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

/** 背单词 request, as `msime_client_vocabulary_review` reads it. */
interface VocabularyRequest {
  directory: string;
  day: string;
  action: VocabularyAction;
  /** 插件目录：其中的单词本插件列进书目。只在声明了 `wordbook_packs` 的形态（2in1）上传，手机不传。 */
  plugins?: string;
}

interface VocabularyAction {
  operation: string;
  word?: string;
  known?: boolean;
  wordbook?: string;
  new_per_day?: number;
  session_limit?: number;
  name?: string;
  text?: string;
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

interface TranslationBody {
  text: string;
  source_lang: string;
  target_lang: string;
}

interface TranslationValue {
  text?: string;
  translation?: string;
  data?: string;
}

interface TranslationDocument {
  code?: number | string;
  error?: Object;
  data?: string | TranslationValue | TranslationValue[];
  translation?: string | TranslationValue | TranslationValue[];
  result?: string | TranslationValue | TranslationValue[];
  translations?: TranslationValue[];
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

interface CredentialHeaders {
  Accept: string;
  'Content-Type': string;
  Authorization?: string;
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
  private context: common.UIAbilityContext = getContext(this) as common.UIAbilityContext;
  private bridge: SettingsBridge = new SettingsBridge();
  // Notices from the MSIME service that the user has not dismissed, newest first; the card shows the first.
  @State notices: NoticeItem[] = [];
  private noticeController: web_webview.WebviewController = new web_webview.WebviewController();
  private noticeAttached: boolean = false;
  // The notice whose body the card's web view holds, so a refresh that changes nothing does not reload it.
  private noticeShown: string = '';

  aboutToAppear(): void {
    this.bridge.attach(this.context);
    this.bridge.useWebview(this.controller);
    // Started here rather than on demand: the page reads the answer synchronously, and the reply
    // carries whether it is ready so the page waits instead of acting on a default.
    this.bridge.prepareOnboarding();
    this.loadNotices();
    EntryAbility.onActivated = (): void => {
      this.bridge.refreshPreferencesIfChanged();
      this.loadNotices();
    };
  }

  /**
   * Fetches the notices when the window opens or comes back to the front, never on a timer: client-core answers from its copy for a minute after a fetch, which is as often as the service wants to be asked.
   */
  private loadNotices(): void {
    const request: NoticesRequest = {
      directory: `${this.context.filesDir}/state/notices`, platform: PLATFORM, channel: 'app'
    };
    client.notices(JSON.stringify(request)).then((reply: string): void => {
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
    .margin({ left: 12, right: 12, top: 8, bottom: 4 })
    .borderRadius(16)
    .backgroundColor($r('sys.color.ohos_id_color_card_bg'))
  }

  aboutToDisappear(): void {
    // The ability outlives this page, so a handler left behind would hold a bridge whose webview
    // has gone.
    EntryAbility.onActivated = undefined;
  }

  build() {
    Column() {
      if (this.notices.length > 0) {
        this.noticeCard()
      }
      Web({ src: $rawfile('settings/index.html'), controller: this.controller })
        .javaScriptAccess(true)
        .domStorageAccess(true)
        .fileAccess(true)
        // Registered through the controller rather than the attribute: the attribute form takes an
        // async list but never settles the promises it hands the page, which is worse than the bug
        // it was meant to fix — the page hangs on its startup screen instead of quietly receiving
        // nothing. onControllerAttached runs before the first load, so the page still finds the
        // object already there rather than racing it.
        .onControllerAttached(() => {
          try {
            this.controller.registerJavaScriptProxy(this.bridge, BRIDGE_NAME,
              ['loadPreferences', 'savePreferences', 'hostCapabilities', 'appVersion',
                'scanSkinCatalog', 'readSkinImage', 'readSkinFont', 'readSkinToolbarCss',
                'dictionaryManifest', 'dictionary', 'cloudDictionaryDownload',
                'typingStatistics', 'openExternalUrl', 'copyText',
                'openSystemKeyboardSettings', 'listVoiceCaptureDevices', 'listFontFamilies',
                'keyboardFeedback', 'onboardingStatus',
                'showInputMethodPicker', 'importSkinFolder', 'startRequest', 'plugins'],
              []);
          } catch (error) {
            KeyboardLog.error(`settings bridge not registered: ${describeError(error)}`);
          }
        })
        .width('100%')
        .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
  }
}

/**
 * What the settings page is allowed to ask of the system.
 *
 * Each method answers the JSON the shared C ABI already returns, so a reply that the Engine refused
 * arrives at the page as the refusal rather than as an exception thrown across the bridge.
 */
class SettingsBridge {
  private stateDirectory: string = '';
  // What the page was last handed, so an activation can tell a changed document from an unchanged
  // one. -1 until the page has loaded once, which never compares equal to a real revision.
  private observedRevision: number = -1;
  private onboardingRequired: boolean = false;
  private onboardingReady: boolean = false;
  // Needed to hand the page a document: the bridge is called from the page, but this one call goes
  // the other way.
  private webview: web_webview.WebviewController | undefined = undefined;
  private context: common.UIAbilityContext | undefined = undefined;
  private accountBridge: HarmonyAccountCloudBridge | undefined = undefined;
  private aiSkins: HarmonyAiSkins | undefined = undefined;
  private cloudDownloadRequest: number = 0;

  useWebview(controller: web_webview.WebviewController): void {
    this.webview = controller;
  }

  attach(context: common.UIAbilityContext): void {
    this.context = context;
    this.stateDirectory = `${context.filesDir}/state`;
    this.accountBridge = new HarmonyAccountCloudBridge(context);
    this.aiSkins = this.accountBridge.aiSkins();
    // The same copy the keyboard makes, by the same rule. The page reaches the dictionary through
    // it too, and two staging rules for one directory is how it came to hold a set matching neither.
    StagedResources.stage(`${context.resourceDir}/engine`, `${context.filesDir}/engine`,
      `${context.filesDir}/engine.ready`);
    // Here as well as in the keyboard, so the page offers Cantonese, Zhuyin and Stroke (see hostCapabilities) before the keyboard has ever run.
    StagedResources.stageLanguageDictionaries(context.resourceDir, context.filesDir);
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
    this.runSkinImport().catch((error: Object): void => {
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
      options.selectMode = picker.DocumentSelectMode.FOLDER;
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
    const failed: (error: Object) => void = (error: Object): void => {
      deliver(JSON.stringify({ ok: false, error: describeError(error) }));
    };
    if (kind === 'account') {
      this.account(payload).then(deliver).catch(failed);
    } else if (kind === 'cloud_dictionary') {
      this.cloudDictionary(payload).then(deliver).catch(failed);
    } else if (kind === 'chat') {
      this.account(payload).then(deliver).catch(failed);
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
    } else if (kind === 'custom_skin_library') {
      this.customSkinLibrary(payload).then(deliver).catch(failed);
    } else if (kind === 'vocabulary_review') {
      this.vocabularyReview(payload).then(deliver).catch(failed);
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
    } else if (kind === 'plugin_import') {
      this.importPlugin(payload).then(deliver).catch((error: Object): void => {
        KeyboardLog.error(`plugin import failed: ${describeError(error)}`);
        deliver(JSON.stringify({ ok: false, error: 'storage' }));
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
      return LocalVoiceModelPolicy.rewrite(client.voiceLocalModelCancel(JSON.stringify(request)));
    }
    if (action.operation === 'remove') {
      const request: LocalVoiceModelRequest = { root: root, id: id };
      return LocalVoiceModelPolicy.rewrite(
        await client.voiceLocalModelRemove(JSON.stringify(request)));
    }
    if (!fs.accessSync(root)) {
      fs.mkdirSync(root, true);
    }
    const mirror: string = LocalVoiceModelPolicy.mirror(client.loadPreferences(this.stateDirectory));
    const request: LocalVoiceModelRequest = { root: root, id: id, mirror: mirror };
    const reply: string = await client.voiceLocalModelInstall(JSON.stringify(request),
      (progress: string): void => this.deliverVoiceModelProgress(progress));
    return LocalVoiceModelPolicy.installReply(reply);
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
   * The 插件 page's pack store and @ name list: `catalog`, `remove`, `load_mentions` and `save_mentions`, answered by `msime_client_plugins` with the same rules and failure codes the desktop shell answers with. The page never names a path: the state root and the bundle's built-in sound packs are filled in here, and an import, which has to wait for the system picker, arrives through `startRequest` as `plugin_import` instead.
   */
  plugins(actionDocument: string): string {
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
    if (!['catalog', 'remove', 'load_mentions', 'save_mentions'].includes(action.operation)) {
      return refused;
    }
    return client.plugins(`{"state_root":${JSON.stringify(this.stateDirectory)},`
      + `"sound_packs":${JSON.stringify(this.soundPacks())},"action":${actionDocument}}`);
  }

  /** The built-in sound packs this bundle ships, where the keyboard's players read them too. */
  private soundPacks(): string {
    return this.context === undefined ? '' : `${this.context.resourceDir}/sound-packs`;
  }

  /**
   * Installs a pack folder or `.zip` file the user picks with the system picker.
   *
   * What the picker hands back is a document URI the native library cannot open, so the pick is copied into a private staging directory first, under a fixed name rather than the picked one, and installed from there by `msime_client_plugins` on a worker thread, which checks it by the same rules as every other host and leaves nothing installed when it refuses. The copy stops at client-core's bounds (`PluginImportPolicy`), so a wrong pick is refused before it is copied whole. The staging copy is removed however the import ends, after the worker has finished with it. The reply is the installed pack, or null when the user closed the picker, which is not a failure.
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
      options.selectMode = folder ? picker.DocumentSelectMode.FOLDER : picker.DocumentSelectMode.FILE;
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
        const file: fs.File = fs.openSync(picked[0], fs.OpenMode.READ_ONLY);
        try {
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
      await fs.copyFile(`${picked}/${name}`, `${destination}/${name}`);
    }
    return null;
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
    if (this.aiSkins === undefined) {
      return JSON.stringify({ ok: false, error: 'ai_skin_unavailable' });
    }
    let action: AiSkinAction;
    try {
      action = JSON.parse(actionDocument) as AiSkinAction;
    } catch {
      return JSON.stringify({ ok: false, error: 'ai_skin_invalid' });
    }
    const id: string = action.request_id ?? '';
    if (action.operation === 'cancel') return this.aiSkins.cancel(id);
    if (action.operation !== 'generate') {
      return JSON.stringify({ ok: false, error: 'ai_skin_invalid' });
    }
    return await this.aiSkins.generate(id, action.prompt ?? '', (completed: number): void => {
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
    if (action.resource_operation === 'store_reply') {
      return await client.communityResourceLibrary(`{"file":${JSON.stringify(file)},`
        + `"action":{"operation":"save_reply","item":${JSON.stringify(action.item ?? null)}}}`);
    }
    if (action.resource_operation === 'remove_reply') {
      const remove: ResourceLibraryRequest = {
        file: file,
        action: { operation: 'remove', id: action.id ?? '' } as ResourceLibraryRemove
      };
      return await client.communityResourceLibrary(JSON.stringify(remove));
    }
    return this.accountBridge.request(actionDocument);
  }

  /**
   * The community skin gallery.
   *
   * Browsing, rating, publishing and withdrawing are one request each and go straight to the
   * account bridge. Downloading is the one that is not: the design has to be fetched over the
   * system HTTPS stack and then put on the keyboard and into the library, and those two local steps
   * are a single native call because a failed import has to end the trial it started.
   *
   * Finishing a trial never touches the network at all. It is the answer to 保留 or 还原 on a skin
   * that is already applied, so it is answered from the device.
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
    if (action.community_operation === 'finish_trial') {
      return this.finishSkinTrial(action);
    }
    if (action.community_operation !== 'download') {
      return this.accountBridge.request(actionDocument);
    }
    return await this.installCommunitySkin(action);
  }

  /**
   * Downloads one design and puts it on the keyboard.
   *
   * The reply shape is the shared one — `{skin, trial}` — because the page uses both: the skin to
   * show in the library it just grew, the trial id to answer 保留 or 还原 with afterwards.
   */
  private async installCommunitySkin(action: CommunitySkinAction): Promise<string> {
    if (this.accountBridge === undefined) {
      return JSON.stringify({ ok: false, error: 'community_unavailable' });
    }
    const downloaded: string = await this.accountBridge.request(JSON.stringify({
      operation: 'community_skin', community_operation: 'download', id: action.id
    }));
    let design: Object;
    try {
      const reply: CommunityDownloadReply = JSON.parse(downloaded) as CommunityDownloadReply;
      if (reply.ok !== true || reply.value === undefined || reply.value === null
        || reply.value.design === undefined) {
        return downloaded.length > 0 ? downloaded
          : JSON.stringify({ ok: false, error: 'community_unavailable' });
      }
      design = reply.value.design;
    } catch {
      return JSON.stringify({ ok: false, error: 'community_unavailable' });
    }
    const installed: string = await client.communitySkinInstall(JSON.stringify({
      directory: this.stateDirectory, id: action.id, name: action.name, design: design
    }));
    // The install wrote the preference document, so the page's copy is now the old one. Same
    // reasoning as the settings sync: a control still showing the previous skin would refuse the
    // next save as a conflict.
    this.refreshPreferencesIfChanged();
    return installed;
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

  /** Read only an image declared by a validated external skin manifest. */
  readSkinImage(id: string, relative: string): string {
    return client.skinResource(JSON.stringify({
      directory: `${this.stateDirectory}/skins`, id: id, relative: relative, kind: 'image'
    }));
  }

  /** Read only a font declared by a validated external skin manifest. */
  readSkinFont(id: string, relative: string): string {
    return client.skinResource(JSON.stringify({
      directory: `${this.stateDirectory}/skins`, id: id, relative: relative, kind: 'font'
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
  savePreferences(expectedRevision: number, document: string): string {
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
    const file: string | null = KeyboardScheme.languageDictionary(scheme);
    return file === null
      || (this.context !== undefined && fs.accessSync(`${this.context.filesDir}/language-dictionaries/${file}`));
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
        SettingsFormFactorCapabilities.resolve(deviceInfo.deviceType);
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
      value['candidate_window_opacity'] = projection.candidateWindowOpacity;
      value['candidate_corner_radius'] = projection.candidateCornerRadius;
      value['input_mode_hud'] = projection.inputModeHud;
      value['key_sound'] = projection.keySound;
      value['music'] = projection.music;
      value['plugin_triggers'] = projection.pluginTriggers;
      value['typing_effects'] = projection.typingEffects;
      value['wordbook_packs'] = projection.wordbookPacks;
      value['symbol_set_packs'] = projection.symbolSetPacks;
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
    return KeyboardSession.shared.dictionary(action, `${this.context.filesDir}/engine`,
      this.stateDirectory);
  }

  /** Account and cloud clipboard requests stay native; the WebView sees only safe JSON DTOs. */
  account(action: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'account_unavailable' }));
    }
    return this.accountBridge.request(action);
  }

  cloudDictionary(action: string): Promise<string> {
    if (this.accountBridge === undefined) {
      return Promise.resolve(JSON.stringify({ ok: false, error: 'account_unavailable' }));
    }
    return this.accountBridge.request(action);
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
   * 背单词: forward one action to the shared review session.
   *
   * The numbered bridge keeps the potentially multi-megabyte import and its file work off the
   * ArkTS thread. The day is resolved here because this process knows the device's timezone.
   *
   * The day is resolved here rather than in the page. This process is the one that knows the
   * device's timezone, and the shared layer never derives a day from one.
   */
  private async vocabularyReview(actionDocument: string): Promise<string> {
    if (actionDocument.length === 0 || actionDocument.length > 8 * 1024 * 1024) {
      return JSON.stringify({ ok: false, error: 'invalid vocabulary review request' });
    }
    try {
      const action: VocabularyAction = JSON.parse(actionDocument) as VocabularyAction;
      if (!['load', 'answer', 'set_settings', 'import', 'remove', 'reset'].includes(action.operation)) {
        return JSON.stringify({ ok: false, error: 'invalid vocabulary review request' });
      }
      const now: Date = new Date();
      const month: string = `${now.getMonth() + 1}`.padStart(2, '0');
      const date: string = `${now.getDate()}`.padStart(2, '0');
      const request: VocabularyRequest = {
        directory: this.stateDirectory,
        day: `${now.getFullYear()}-${month}-${date}`,
        action: action,
      };
      // 与设置页拿到的 `wordbook_packs` 能力同一个来源：没有这项能力的形态不列单词本插件。
      if (SettingsFormFactorCapabilities.resolve(deviceInfo.deviceType).wordbookPacks) {
        request.plugins = `${this.stateDirectory}/plugins`;
      }
      return await client.vocabularyReview(JSON.stringify(request));
    } catch (error) {
      KeyboardLog.error(`vocabulary review failed: ${describeError(error)}`);
      return JSON.stringify({ ok: false, error: 'vocabulary review failed' });
    }
  }

  typingStatistics(actionDocument: string): string {
    if (actionDocument.length === 0 || actionDocument.length > 4096) {
      return JSON.stringify({ ok: false, error: 'invalid typing statistics request' });
    }
    try {
      const action: TypingStatisticsAction = JSON.parse(actionDocument) as TypingStatisticsAction;
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
      if (reply.value === undefined || reply.value === null) {
        return JSON.stringify({ ok: false, error: 'invalid typing statistics response' });
      }
      let lastWrittenMs: number | null = null;
      let available: boolean = false;
      const file: string = `${this.stateDirectory}/typing-statistics.json`;
      try {
        if (fs.accessSync(file)) {
          available = true;
          lastWrittenMs = fs.statSync(file).mtime;
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
    const endpoint: string | null = AiModelCatalogPolicy.modelsUrl(request.endpoint);
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
    const body: TranslationBody = { text: '测试', source_lang: 'ZH', target_lang: 'EN' };
    const headers: CredentialHeaders = {
      Accept: 'application/json', 'Content-Type': 'application/json'
    };
    if (apiKey.length > 0) headers.Authorization = `Bearer ${apiKey}`;
    const request = http.createHttp();
    installNoRedirectGuard(request);
    try {
      const response: http.HttpResponse = await request.request(endpoint, {
        method: http.RequestMethod.POST,
        header: headers,
        connectTimeout: 5000,
        readTimeout: 15000,
        extraData: JSON.stringify(body)
      });
      if (response.responseCode < 200 || response.responseCode >= 300
        || typeof response.result !== 'string' || utf8Length(response.result) > 256 * 1024) {
        return this.credentialReply(false, '测试失败：翻译服务未返回有效译文。');
      }
      const document: TranslationDocument = JSON.parse(response.result) as TranslationDocument;
      const translated: string | null = this.translationText(document);
      return translated === null
        ? this.credentialReply(false, '测试失败：翻译服务未返回有效译文。')
        : this.credentialReply(true, '连接成功，翻译服务配置有效。');
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
      || region.length > 64 || Array.from(region).some((character: string): boolean => {
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
      if (response.responseCode === 401 || response.responseCode === 403) {
        return this.credentialReply(false, '认证失败，请检查 API Key 和访问权限。');
      }
      if (response.responseCode === 429) {
        return this.credentialReply(false, '服务限流或额度不足，请稍后重试。');
      }
      return this.credentialReply(false, '服务拒绝请求，请检查接口地址和模型配置。');
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
    if (!['openai', 'siliconflow', 'groq', 'everyapi', 'mistral'].includes(provider) || endpoint === null
      || !this.validCredentialSecret(token, 8192) || model.length === 0
      || utf8Length(model) > 256 || TextPolicy.hasControl(model)) {
      return this.credentialReply(false, '请填写有效的 HTTPS 转写接口地址、API Key 和模型名。');
    }
    const boundary: string = '----MSIMEHarmonyCredentialProbe';
    const body: ArrayBuffer = this.silentMultipart(boundary, model);
    const request = http.createHttp();
    installNoRedirectGuard(request);
    try {
      const response: http.HttpResponse = await request.request(endpoint, {
        method: http.RequestMethod.POST,
        header: { Accept: 'application/json', Authorization: `Bearer ${token}`,
          'Content-Type': `multipart/form-data; boundary=${boundary}` },
        connectTimeout: 5000,
        readTimeout: 15000,
        extraData: body
      });
      if (response.responseCode >= 200 && response.responseCode < 300) {
        return this.credentialReply(true, '连接成功，API Key 和模型配置有效。');
      }
      if (response.responseCode === 401 || response.responseCode === 403) {
        return this.credentialReply(false, '认证失败，请检查 API Key 和访问权限。');
      }
      if (response.responseCode === 429) {
        return this.credentialReply(false, '服务限流或额度不足，请稍后重试。');
      }
      return this.credentialReply(false, '转写测试未成功，请检查接口地址、模型和服务配置。');
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
        const startPayload: ArrayBuffer = encoder.encode(JSON.stringify(start)).buffer as ArrayBuffer;
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
    const prefix: Uint8Array = encoder.encode(`--${boundary}\r\nContent-Disposition: form-data; name="file"; filename="credential-test.wav"\r\nContent-Type: audio/wav\r\n\r\n`);
    const modelPart: Uint8Array = encoder.encode(`\r\n--${boundary}\r\nContent-Disposition: form-data; name="model"\r\n\r\n${model}\r\n--${boundary}--\r\n`);
    const output: ArrayBuffer = new ArrayBuffer(prefix.length + wav.length + modelPart.length);
    const bytes: Uint8Array = new Uint8Array(output);
    bytes.set(prefix, 0);
    bytes.set(wav, prefix.length);
    bytes.set(modelPart, prefix.length + wav.length);
    return output;
  }

  private writeAscii(bytes: Uint8Array, offset: number, value: string): void {
    const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
    bytes.set(encoder.encode(value), offset);
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

  private translationText(document: TranslationDocument): string | null {
    if (document.error !== undefined) return null;
    if (document.code !== undefined && document.code !== 200 && document.code !== '200') return null;
    const values: Array<string | TranslationValue | TranslationValue[]> = [];
    if (document.data !== undefined) values.push(document.data);
    if (document.translation !== undefined) values.push(document.translation);
    if (document.result !== undefined) values.push(document.result);
    if (document.translations !== undefined) values.push(document.translations);
    for (const value of values) {
      if (typeof value === 'string' && value.trim().length > 0) return value.trim();
      if (Array.isArray(value) && value.length > 0) {
        const first: TranslationValue = value[0];
        const text: string | undefined = first.text ?? first.translation ?? first.data;
        if (text !== undefined && text.trim().length > 0) return text.trim();
      } else if (typeof value === 'object' && value !== null) {
        const objectValue: TranslationValue = value as TranslationValue;
        const text: string | undefined = objectValue.text ?? objectValue.translation ?? objectValue.data;
        if (text !== undefined && text.trim().length > 0) return text.trim();
      }
    }
    return null;
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
      && !Array.from(value).every((character: string): boolean => character === '*');
  }

  private validAiEndpoint(value: string): string | null {
    const endpoint: string = value.trim();
    if (!TextPolicy.validSecureAuthority(endpoint, true)) return null;
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
   * The settings page is a second writer of the same file the keyboard reads at startup, which is
   * why a change here reaches the keyboard the next time it starts rather than mid-session.
   */
  keyboardFeedback(request: string): string {
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
    if (action.operation === 'save') {
      const settings: FeedbackSettings =
        KeyboardFeedbackBridge.fromShared(action.settings ?? null);
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

  /** One buzz at the strength being previewed. A device without a vibrator simply does nothing. */
  private previewFeedback(strength: string): void {
    try {
      vibrator.startVibration(
        { type: 'time', duration: KeyboardFeedbackBridge.previewDuration(strength) },
        { usage: 'physicalFeedback' }).catch((error: Object): void => {
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
      const families: string[] = font.getSystemFontList().filter((name: string): boolean =>
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
      ok: true, value: this.onboardingRequired, ready: this.onboardingReady, error: ''
    } as OnboardingReply);
  }

  /** Computes the answer the page will read. Called before the page is shown. */
  async prepareOnboarding(): Promise<void> {
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
    const state: OnboardingState = {
      enabled: enabled, currentBundle: current, ownBundle: own
    };
    KeyboardLog.info(`onboarding: ${OnboardingStatePolicy.describe(state)}`);
    this.onboardingRequired = OnboardingStatePolicy.required(state);
    this.onboardingReady = true;
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
    inputMethod.getSetting().showOptionalInputMethods().catch((error: Object): void => {
      KeyboardLog.error(`keyboard picker would not open: ${describeError(error)}`);
    });
    return JSON.stringify({ ok: true, value: true, ready: true, error: '' } as OnboardingReply);
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
