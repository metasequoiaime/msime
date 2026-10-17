import fs from '@ohos.file.fs';
import common from '@ohos.app.ability.common';
import picker from '@ohos.file.picker';
import util from '@ohos.util';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { KeyboardSession } from '../keyboard/KeyboardSession';
import { KeyboardFeedbackBridge, MobileKeyboardFeedback } from '../keyboard/KeyboardFeedbackBridge';
import { TypingStatisticsPolicy } from '../keyboard/TypingStatisticsPolicy';
import { AccountSessionAdoption } from './AccountSessionAdoption';
import { HarmonyAccountIdentity } from './HarmonyAccountIdentity';
import { HarmonyGoogleSignIn } from './HarmonyGoogleSignIn';
import {
  RequestPlan, SESSION_CHANGING_OPERATIONS, accountRequest, aiSkinRequest, appliedFeedbackText,
  cloudClipboardRequest, cloudDictionaryFileRequest, cloudDictionaryRequest, communityRequest, hostAccountRequest,
  preferencesPutAction, replyError, replyValueText, settingsApplyRequest, settingsExportRequest, statusUserId,
  typingUploadReply, typingUploadRequest
} from './HarmonyAccountRequests';

/** 页面动作里只读操作名，其余字段由 `HarmonyAccountRequests` 和 Rust 检查。 */
interface OperationOnly {
  operation?: string;
}

interface SnapshotTokenAction {
  operation?: string;
  token?: string;
}

interface DictionaryExportAction {
  operation?: string;
  kind?: string;
  format?: string;
}

interface PlainAction {
  operation: string;
}

interface TokenAction {
  operation: string;
  token: string;
}

interface PathAction {
  operation: string;
  path: string;
}

interface DictionaryExportFileAction {
  operation: string;
  kind: string;
  format: string;
  path: string;
}

interface RestoreFileAction {
  operation: string;
  path: string;
  expected_sha256: string;
  revision: number;
}

interface UserIdAction {
  operation: string;
  user_id: string;
}

interface SnapshotCancelAction {
  operation: string;
  directory: string;
  account_id: string;
}

interface Envelope {
  ok?: boolean;
  error?: string;
}

interface SnapshotMetadataValue {
  sha256?: string;
}

interface SnapshotExportValue {
  filename?: string;
  snapshot?: Object;
}

interface RestorePreviewValue {
  snapshot?: SnapshotMetadataValue;
  expectedRevision?: number;
}

interface SavedValue {
  saved: boolean;
  snapshot?: Object;
  previewToken?: string;
  expectedRevision?: number;
}

interface SavedReply {
  ok: boolean;
  value: SavedValue;
}

interface EmptyValue {}

interface EmptyReply {
  ok: boolean;
  value: EmptyValue;
}

interface NullReply {
  ok: boolean;
  value: null;
}

/** 设置同步只交换这三个按键反馈字段，与 `HostKeyboardFeedback` 相同。 */
interface FeedbackValues {
  soundEnabled: boolean;
  hapticsEnabled: boolean;
  hapticStrength: string;
}

const DICTIONARY_KINDS: string[] = ['pinyin', 'wubi', 'wubi98', 'quick', 'english'];
const DICTIONARY_FORMATS: string[] = ['standard', 'windows'];

/**
 * 设置应用的账号与云端业务（#8201 第 2 步）。会话、刷新令牌和全部 HTTP 都在 Rust 的目录模式里，目录是本沙箱的 `files/state`，与键盘沙箱之间的会话镜像经 `AccountSessionAdoption`；这里只做 Rust 做不了的那部分：
 *
 * - 页面能发哪些操作（`HarmonyAccountRequests` 的名单），带路径的字段只由这里填；
 * - 每个认证请求之前先等本进程的会话镜像做完；
 * - 登录、退出、注销前后取消旧账号排队中的词库快照；
 * - 文件选择器：选择器给的是 URI，Rust 要的是路径，所以导出先写进私有目录再由这里复制到用户选的位置，恢复先把选中的文件复制进私有目录再交给 Rust；
 * - Google 回环登录的 socket 和浏览器（`HarmonyGoogleSignIn`）；
 * - 设置同步里本宿主才有的按键反馈文件。
 */
export class HarmonyAccountCloudBridge {
  private readonly context: common.UIAbilityContext;
  private readonly directory: string;
  private readonly google: HarmonyGoogleSignIn;
  private restoreFile: string = '';
  private restoreToken: string = '';
  private restoreAccountId: string = '';
  private restoreSha256: string = '';
  private restoreRevision: number = -1;

  constructor(context: common.UIAbilityContext) {
    this.context = context;
    this.directory = `${context.filesDir}/state`;
    try {
      if (!fs.accessSync(this.directory)) fs.mkdirSync(this.directory, true);
    } catch {
      // 目录建不出来时 Rust 的每个请求都会报 account_storage。
    }
    // 回环流程里申请 challenge 和提交授权码是宿主自己发的账号操作，与页面的操作走同一个目录和 User-Agent。
    this.google = new HarmonyGoogleSignIn(context, (action: string): Promise<string> => this.hostAccount(action));
  }

  /** 本沙箱里登录的账号，没有登录时为 null。只读本地会话文件，可以在主线程上调用。 */
  currentUserId(): string | null {
    try {
      return statusUserId(client.accountStatus(this.directory));
    } catch (error) {
      KeyboardLog.error(`account status unreadable: ${describeError(error)}`);
      return null;
    }
  }

  /** 页面经「账号」和「对话」通道发来的操作。 */
  async request(action: string): Promise<string> {
    const operation: string = HarmonyAccountCloudBridge.operationOf(action);
    if (operation === 'google_cancel') {
      this.google.cancel();
      const reply: NullReply = { ok: true, value: null };
      return JSON.stringify(reply);
    }
    let plan: RequestPlan | null = null;
    if (operation !== 'google_sign_in') {
      plan = accountRequest(this.directory, HarmonyAccountIdentity.clientAgent(), action);
      if (plan.refusal.length > 0) return plan.refusal;
    }
    await AccountSessionAdoption.beforeRequest();
    const accountId: string | null = this.currentUserId();
    if (SESSION_CHANGING_OPERATIONS.includes(operation)) {
      this.clearRestorePreview();
      if (accountId !== null && !this.cancelQueueForAccount(accountId)) {
        return HarmonyAccountCloudBridge.failure('snapshot_unavailable');
      }
    }
    let reply: string;
    if (plan === null) {
      reply = await this.google.signIn();
    } else if (operation === 'chat') {
      reply = await client.accountThread(plan.request);
    } else {
      reply = await client.account(plan.request);
    }
    // 操作期间换了账号（登录了另一个、退出、注销）时，旧账号排队中的快照不再属于任何人。
    if (accountId !== null && replyError(reply, 'account_unavailable').length === 0
      && this.currentUserId() !== accountId) {
      this.cancelQueueForAccount(accountId);
    }
    return reply;
  }

  /** 宿主自己发的账号操作：Google 回环登录的 challenge 和授权码。 */
  private async hostAccount(action: string): Promise<string> {
    const request: string | null =
      hostAccountRequest(this.directory, HarmonyAccountIdentity.clientAgent(), action);
    if (request === null) return HarmonyAccountCloudBridge.failure('account_invalid');
    await AccountSessionAdoption.beforeRequest();
    return await client.account(request);
  }

  /** 社区的皮肤、词包与回复模板。`finish_trial`、`store_reply`、`remove_reply` 不联网，由 `Settings` 在这之前处理。 */
  async community(action: string): Promise<string> {
    const plan: RequestPlan = communityRequest(this.directory, this.directory, action);
    if (plan.refusal.length > 0) return plan.refusal;
    await AccountSessionAdoption.beforeRequest();
    return await client.community(plan.request);
  }

  /** 设置页的云剪贴板面板。 */
  async cloudClipboard(action: string): Promise<string> {
    const plan: RequestPlan =
      cloudClipboardRequest(this.directory, HarmonyAccountIdentity.deviceName(), action);
    if (plan.refusal.length > 0) return plan.refusal;
    await AccountSessionAdoption.beforeRequest();
    return await client.accountCloudClipboard(plan.request);
  }

  /** AI 皮肤：`generate` 每完成一张插画经 `progress` 报一次，`cancel` 立即返回。 */
  async aiSkin(action: string, progress: (completed: number) => void): Promise<string> {
    const plan: RequestPlan = aiSkinRequest(this.directory, action);
    if (plan.refusal.length > 0) return plan.refusal;
    if (plan.operation === 'cancel') return client.aiSkinCancel(plan.request);
    await AccountSessionAdoption.beforeRequest();
    return await client.aiSkinGenerate(plan.request, progress);
  }

  /** 打字统计「上传到云端」。节流、串行和状态文件都在 Rust 里。 */
  async typingUpload(action: string): Promise<string> {
    const plan: RequestPlan = typingUploadRequest(this.directory, HarmonyAccountIdentity.clientAgent(),
      this.directory, TypingStatisticsPolicy.day(new Date()), action);
    if (plan.refusal.length > 0) return plan.refusal;
    await AccountSessionAdoption.beforeRequest();
    return typingUploadReply(await client.account(plan.request));
  }

  /** 云词库面板的短请求；`export` 经文件选择器保存。 */
  async cloudDictionary(action: string, stateRoot: string): Promise<string> {
    if (HarmonyAccountCloudBridge.operationOf(action) === 'export') {
      return await this.exportDictionary(action, stateRoot);
    }
    const plan: RequestPlan = cloudDictionaryRequest(this.directory, action);
    if (plan.refusal.length > 0) return plan.refusal;
    await AccountSessionAdoption.beforeRequest();
    return await client.cloudDictionary(plan.request);
  }

  /**
   * 设置同步，本机唯一写到设备上的账号操作。映射在 Rust（`msime_client_account_harmony_settings_export`、`_apply`），这里补上 Rust 两个映射入口不做的：核对账号（它们不读会话）、取字段表和云端文档、读写按键反馈文件。
   */
  async preferencesSync(actionDocument: string, stateRoot: string): Promise<string> {
    let action: UserIdAction;
    try {
      action = JSON.parse(actionDocument) as UserIdAction;
    } catch {
      return HarmonyAccountCloudBridge.failure('account_invalid');
    }
    await AccountSessionAdoption.beforeRequest();
    if (action.operation === 'preferences_schema' || action.operation === 'preferences_load') {
      const plain: PlainAction = { operation: action.operation };
      return await this.accountAction(JSON.stringify(plain));
    }
    if (action.operation === 'preferences_upload') return await this.uploadPreferences(stateRoot);
    if (action.operation === 'preferences_apply') {
      return await this.applyPreferences(typeof action.user_id === 'string' ? action.user_id : '', stateRoot);
    }
    return HarmonyAccountCloudBridge.failure('account_invalid');
  }

  private async accountAction(actionDocument: string): Promise<string> {
    const request: string | null =
      hostAccountRequest(this.directory, HarmonyAccountIdentity.clientAgent(), actionDocument);
    return request === null ? HarmonyAccountCloudBridge.failure('account_invalid') : await client.account(request);
  }

  private feedbackDocument(stateRoot: string): string {
    const shared: MobileKeyboardFeedback = KeyboardFeedbackBridge.toShared(KeyboardSession.feedbackIn(stateRoot));
    const values: FeedbackValues = {
      soundEnabled: shared.soundEnabled,
      hapticsEnabled: shared.hapticsEnabled,
      hapticStrength: shared.hapticStrength
    };
    return JSON.stringify(values);
  }

  /** 本机的设置合并进云端文档后整份上传，绑定开始时登录的账号：中途退出或换号时 Rust 回 `account_cancelled`。 */
  private async uploadPreferences(stateRoot: string): Promise<string> {
    const userId: string | null = this.currentUserId();
    if (userId === null) return HarmonyAccountCloudBridge.failure('account_unauthorized');
    const schemaReply: string = await this.accountAction('{"operation":"preferences_schema"}');
    const schema: string | null = replyValueText(schemaReply);
    if (schema === null) return schemaReply;
    const cloudReply: string = await this.accountAction('{"operation":"preferences_load"}');
    const cloud: string | null = replyValueText(cloudReply);
    if (cloud === null) return cloudReply;
    const exportRequest: string | null =
      settingsExportRequest(stateRoot, this.feedbackDocument(stateRoot), schema, cloud);
    if (exportRequest === null) return HarmonyAccountCloudBridge.failure('account_unavailable');
    const exported: string = await client.harmonySettingsExport(exportRequest);
    const exportedValue: string | null = replyValueText(exported);
    if (exportedValue === null) return exported;
    const put: string | null = preferencesPutAction(exportedValue, userId);
    if (put === null) return HarmonyAccountCloudBridge.failure('account_unavailable');
    return await this.accountAction(put);
  }

  /**
   * 账号的设置写到本机。页面给的账号先与本机登录的核对；下载以后再核对一次（Rust 的应用入口不读会话），这是写入之前最后一个还来得及说「账号变了」的地方。按键反馈总是带上：文档和字段表里都有这三个键而不带时 Rust 整份拒绝。
   */
  private async applyPreferences(userId: string, stateRoot: string): Promise<string> {
    if (userId.length === 0 || this.currentUserId() !== userId) {
      return HarmonyAccountCloudBridge.failure('account_unauthorized');
    }
    const schemaReply: string = await this.accountAction('{"operation":"preferences_schema"}');
    const schema: string | null = replyValueText(schemaReply);
    if (schema === null) return schemaReply;
    const cloudReply: string = await this.accountAction('{"operation":"preferences_load"}');
    const cloud: string | null = replyValueText(cloudReply);
    if (cloud === null) return cloudReply;
    if (this.currentUserId() !== userId) return HarmonyAccountCloudBridge.failure('account_cancelled');
    const applyRequest: string | null =
      settingsApplyRequest(stateRoot, cloud, schema, this.feedbackDocument(stateRoot));
    if (applyRequest === null) return HarmonyAccountCloudBridge.failure('account_unavailable');
    const applied: string = await client.harmonySettingsApply(applyRequest);
    if (replyError(applied, 'account_unavailable').length > 0) return applied;
    // 反馈文件在偏好之后写，写不进去时如实报告、不回滚已经写好的偏好；再点一次是无害的。
    const feedback: string | null = appliedFeedbackText(applied);
    if (feedback !== null) {
      const shared: MobileKeyboardFeedback = JSON.parse(feedback) as MobileKeyboardFeedback;
      // 滑行输入和手写设置与账号共用文件但不随账号同步，账号不带的字段都由已存储的设置填充。
      const written: boolean = KeyboardSession.saveFeedbackIn(stateRoot,
        KeyboardFeedbackBridge.fromShared(shared, KeyboardSession.feedbackIn(stateRoot)));
      if (!written) return HarmonyAccountCloudBridge.failure('account_storage');
    }
    const reply: EmptyReply = { ok: true, value: {} as EmptyValue };
    return JSON.stringify(reply);
  }

  /**
   * 整份云词库快照，与 Rust 的 `snapshot_*`、`snapshot_*_file` 逐项对应（迁移差异 M28）：预览、入列、状态、取消原样交给 Rust；导出和恢复要经文件选择器，由这里在选择器的 URI 和私有目录之间复制。
   */
  async snapshot(actionDocument: string, resources: string, stateRoot: string): Promise<string> {
    let action: SnapshotTokenAction;
    try {
      action = JSON.parse(actionDocument) as SnapshotTokenAction;
    } catch {
      return HarmonyAccountCloudBridge.failure('snapshot_invalid');
    }
    const operation: string = typeof action.operation === 'string' ? action.operation : '';
    if (operation === 'snapshot_restore_cancel') {
      this.clearRestorePreview();
      const reply: EmptyReply = { ok: true, value: {} as EmptyValue };
      return JSON.stringify(reply);
    }
    await AccountSessionAdoption.beforeRequest();
    switch (operation) {
      case 'snapshot_status':
      case 'snapshot_preview':
      case 'snapshot_cancel': {
        const plain: PlainAction = { operation: operation };
        return await this.snapshotRequest(JSON.stringify(plain), resources, stateRoot);
      }
      case 'snapshot_enqueue': {
        const enqueue: TokenAction = { operation: operation, token: typeof action.token === 'string' ? action.token : '' };
        return await this.snapshotRequest(JSON.stringify(enqueue), resources, stateRoot);
      }
      case 'snapshot_export':
        return await this.exportSnapshot(stateRoot);
      case 'snapshot_restore_preview':
        return await this.restorePreview(stateRoot);
      case 'snapshot_restore_native':
        return await this.restoreNative(typeof action.token === 'string' ? action.token : '', stateRoot);
      default:
        return HarmonyAccountCloudBridge.failure('snapshot_unavailable');
    }
  }

  /** 设置应用私有的中转目录：Rust 第一次用到时清掉上次运行留下的中转文件，所以只有这一个进程用它。 */
  private snapshotDirectory(stateRoot: string): string {
    return `${stateRoot}/cloud-dictionary-transfer`;
  }

  private hostOptions(resources: string, stateRoot: string): string | null {
    try {
      return replyValueText(client.prepareHost(JSON.stringify(KeyboardSession.prepareRequest(resources, stateRoot))));
    } catch (error) {
      KeyboardLog.error(`snapshot options unavailable: ${describeError(error)}`);
      return null;
    }
  }

  private async snapshotRequest(actionDocument: string, resources: string, stateRoot: string): Promise<string> {
    const options: string | null = this.hostOptions(resources, stateRoot);
    if (options === null) return HarmonyAccountCloudBridge.failure('snapshot_unavailable');
    const request: string | null =
      cloudDictionaryFileRequest(this.directory, this.snapshotDirectory(stateRoot), options, actionDocument);
    if (request === null) return HarmonyAccountCloudBridge.failure('snapshot_invalid');
    return await client.cloudDictionarySnapshot(request);
  }

  /** 只要路径、不要 HostOptions 的文件请求：导出、恢复的预览和恢复本身。 */
  private async fileRequest(actionDocument: string, stateRoot: string): Promise<string> {
    const request: string | null =
      cloudDictionaryFileRequest(this.directory, this.snapshotDirectory(stateRoot), null, actionDocument);
    if (request === null) return HarmonyAccountCloudBridge.failure('snapshot_invalid');
    return await client.cloudDictionarySnapshot(request);
  }

  /** 私有目录里一个新文件的路径；目录不安全（符号链接、不是目录）时为 null。 */
  private privateFile(stateRoot: string, name: string, suffix: string): string | null {
    const directory: string = `${stateRoot}/${name}`;
    try {
      if (!this.safeDirectory(stateRoot)) return null;
      if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);
      if (!this.safeDirectory(directory)) return null;
      return `${directory}/${util.generateRandomUUID(false)}${suffix}`;
    } catch (error) {
      KeyboardLog.error(`private transfer directory unavailable: ${describeError(error)}`);
      return null;
    }
  }

  private static removeFile(path: string): void {
    try {
      if (path.length > 0 && fs.accessSync(path)) fs.unlinkSync(path);
    } catch {
      // 已经不在就是想要的结果。
    }
  }

  /** 选择器给的保存位置；用户取消时为空串。 */
  private async saveLocation(filename: string, suffix: string): Promise<string> {
    const options: picker.DocumentSaveOptions = new picker.DocumentSaveOptions();
    options.newFileNames = [filename];
    options.fileSuffixChoices = [suffix];
    const selected: string[] = await new picker.DocumentViewPicker(this.context).save(options);
    return selected.length === 0 ? '' : selected[0];
  }

  /** 先由 Rust 下载并校验进私有目录，再问用户存在哪里；复制失败时删掉选中的目标，私有文件总会删掉。 */
  private async copyOut(staging: string, filename: string, suffix: string): Promise<string> {
    let destination: string = '';
    try {
      destination = await this.saveLocation(filename, suffix);
      if (destination.length === 0) return '';
      await fs.copyFile(staging, destination);
      return destination;
    } catch (error) {
      HarmonyAccountCloudBridge.removeFile(destination);
      throw error as Error;
    }
  }

  private async exportSnapshot(stateRoot: string): Promise<string> {
    const staging: string | null = this.privateFile(stateRoot, 'cloud-dictionary-exports', '.ndjson');
    if (staging === null) return HarmonyAccountCloudBridge.failure('account_storage');
    try {
      const exportAction: PathAction = { operation: 'snapshot_export_file', path: staging };
      const reply: string = await this.fileRequest(JSON.stringify(exportAction), stateRoot);
      const valueText: string | null = replyValueText(reply);
      if (valueText === null) return reply;
      const value: SnapshotExportValue = JSON.parse(valueText) as SnapshotExportValue;
      const filename: string = typeof value.filename === 'string' && value.filename.length > 0
        ? value.filename : 'msime-dictionary-snapshot.ndjson';
      let saved: boolean = false;
      try {
        saved = (await this.copyOut(staging, filename, 'NDJSON|.ndjson')).length > 0;
      } catch {
        return HarmonyAccountCloudBridge.failure('account_storage');
      }
      const result: SavedReply = { ok: true, value: { saved: saved, snapshot: value.snapshot } };
      return JSON.stringify(result);
    } finally {
      HarmonyAccountCloudBridge.removeFile(staging);
    }
  }

  /** 单个云词库导出：Rust 的 `export_file` 写进私有目录，建议文件名沿用 `msime-<kind>-dictionary.tsv`。 */
  private async exportDictionary(actionDocument: string, stateRoot: string): Promise<string> {
    let action: DictionaryExportAction;
    try {
      action = JSON.parse(actionDocument) as DictionaryExportAction;
    } catch {
      return HarmonyAccountCloudBridge.failure('invalid_cloud_dictionary');
    }
    const kind: string = typeof action.kind === 'string' ? action.kind : '';
    const format: string = typeof action.format === 'string' ? action.format : '';
    // 种类会进建议文件名，先按 Rust 认的五种挡住别的内容。
    if (!DICTIONARY_KINDS.includes(kind) || !DICTIONARY_FORMATS.includes(format)) {
      return HarmonyAccountCloudBridge.failure('invalid_cloud_dictionary');
    }
    const staging: string | null = this.privateFile(stateRoot, 'dictionary-exports', '.tsv');
    if (staging === null) return HarmonyAccountCloudBridge.failure('account_storage');
    try {
      await AccountSessionAdoption.beforeRequest();
      const exportAction: DictionaryExportFileAction =
        { operation: 'export_file', kind: kind, format: format, path: staging };
      const reply: string = await this.fileRequest(JSON.stringify(exportAction), stateRoot);
      if (replyError(reply, 'account_unavailable').length > 0) return reply;
      let saved: boolean = false;
      try {
        saved = (await this.copyOut(staging, `msime-${kind}-dictionary.tsv`, 'TSV|.tsv')).length > 0;
      } catch {
        return HarmonyAccountCloudBridge.failure('account_storage');
      }
      const result: SavedReply = { ok: true, value: { saved: saved } };
      return JSON.stringify(result);
    } finally {
      HarmonyAccountCloudBridge.removeFile(staging);
    }
  }

  /**
   * 选一份备份文件，复制进私有目录交给 Rust 校验。私有副本留到恢复或取消：Rust 恢复时按路径再读一次，并核对它与预览时相同。预览和恢复之间换了账号就作废，不能把这份备份恢复进另一个账号。
   */
  private async restorePreview(stateRoot: string): Promise<string> {
    const accountId: string | null = this.currentUserId();
    if (accountId === null) return HarmonyAccountCloudBridge.failure('account_unauthorized');
    this.clearRestorePreview();
    try {
      const options: picker.DocumentSelectOptions = new picker.DocumentSelectOptions();
      options.maxSelectNumber = 1;
      options.fileSuffixFilters = ['NDJSON|.ndjson'];
      const selected: string[] = await new picker.DocumentViewPicker(this.context).select(options);
      if (selected.length === 0) {
        const cancelled: SavedReply = { ok: true, value: { saved: false } };
        return JSON.stringify(cancelled);
      }
      if (this.currentUserId() !== accountId) return HarmonyAccountCloudBridge.failure('account_cancelled');
      const copy: string | null = this.privateFile(stateRoot, 'cloud-restore', '.ndjson');
      if (copy === null) return HarmonyAccountCloudBridge.failure('account_storage');
      this.restoreFile = copy;
      if (!(await this.copyIn(selected[0], copy))) {
        this.clearRestorePreview();
        return HarmonyAccountCloudBridge.failure('snapshot_invalid');
      }
      const previewAction: PathAction = { operation: 'snapshot_restore_preview_file', path: copy };
      const reply: string = await this.fileRequest(JSON.stringify(previewAction), stateRoot);
      const valueText: string | null = replyValueText(reply);
      if (valueText === null) {
        this.clearRestorePreview();
        return reply;
      }
      const value: RestorePreviewValue = JSON.parse(valueText) as RestorePreviewValue;
      const sha256: string = value.snapshot !== undefined && typeof value.snapshot.sha256 === 'string'
        ? value.snapshot.sha256 : '';
      const revision: number = typeof value.expectedRevision === 'number' ? value.expectedRevision : -1;
      if (sha256.length === 0 || revision < 0) {
        this.clearRestorePreview();
        return HarmonyAccountCloudBridge.failure('snapshot_invalid');
      }
      if (this.currentUserId() !== accountId) {
        this.clearRestorePreview();
        return HarmonyAccountCloudBridge.failure('account_cancelled');
      }
      this.restoreAccountId = accountId;
      this.restoreSha256 = sha256;
      this.restoreRevision = revision;
      this.restoreToken = util.generateRandomUUID(false);
      const result: SavedReply = {
        ok: true,
        value: { saved: true, previewToken: this.restoreToken, snapshot: value.snapshot, expectedRevision: revision }
      };
      return JSON.stringify(result);
    } catch (error) {
      KeyboardLog.error(`snapshot restore preview failed: ${describeError(error)}`);
      this.clearRestorePreview();
      return HarmonyAccountCloudBridge.failure('snapshot_unavailable');
    }
  }

  /** 把选择器给的文件复制进私有路径：只认普通文件、不跟随符号链接，大小在 Rust 的 512 MiB 以内且复制完整。 */
  private async copyIn(source: string, destination: string): Promise<boolean> {
    const sourceFile: fs.File = fs.openSync(source, fs.OpenMode.READ_ONLY);
    try {
      const sourcePathStat: fs.Stat = fs.lstatSync(source);
      const sourceFdStat: fs.Stat = fs.statSync(sourceFile.fd);
      if (sourcePathStat.isSymbolicLink() || !sourcePathStat.isFile()
        || !sourceFdStat.isFile() || sourceFdStat.ino !== sourcePathStat.ino) {
        return false;
      }
      const bytes: number = sourceFdStat.size;
      if (bytes <= 0 || bytes > 512 * 1024 * 1024) return false;
      await fs.copyFile(sourceFile.fd, destination);
      return fs.statSync(destination).size === bytes;
    } finally {
      fs.closeSync(sourceFile);
    }
  }

  private async restoreNative(token: string, stateRoot: string): Promise<string> {
    if (token.length === 0 || token !== this.restoreToken || this.restoreFile.length === 0
      || this.restoreSha256.length === 0 || this.restoreRevision < 0 || this.restoreAccountId.length === 0) {
      return HarmonyAccountCloudBridge.failure('snapshot_invalid');
    }
    try {
      if (this.currentUserId() !== this.restoreAccountId) {
        return HarmonyAccountCloudBridge.failure('account_cancelled');
      }
      const restoreAction: RestoreFileAction = {
        operation: 'snapshot_restore_file', path: this.restoreFile,
        expected_sha256: this.restoreSha256, revision: this.restoreRevision
      };
      return await this.fileRequest(JSON.stringify(restoreAction), stateRoot);
    } finally {
      this.clearRestorePreview();
    }
  }

  private clearRestorePreview(): void {
    HarmonyAccountCloudBridge.removeFile(this.restoreFile);
    this.restoreFile = '';
    this.restoreToken = '';
    this.restoreAccountId = '';
    this.restoreSha256 = '';
    this.restoreRevision = -1;
  }

  /** 取消某个账号排队中的词库快照；队列与激活共用一把锁，返回以后这个账号的快照不会再被应用。 */
  private cancelQueueForAccount(accountId: string): boolean {
    try {
      const cancel: SnapshotCancelAction = {
        operation: 'cancel', directory: `${this.directory}/dictionary-snapshots`, account_id: accountId
      };
      const reply: Envelope = JSON.parse(client.snapshotQueue(JSON.stringify(cancel))) as Envelope;
      return reply.ok === true;
    } catch {
      return false;
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

  private static operationOf(action: string): string {
    try {
      const parsed: OperationOnly = JSON.parse(action) as OperationOnly;
      return typeof parsed.operation === 'string' ? parsed.operation : '';
    } catch {
      return '';
    }
  }

  private static failure(code: string): string {
    const reply: Envelope = { ok: false, error: code };
    return JSON.stringify(reply);
  }
}
