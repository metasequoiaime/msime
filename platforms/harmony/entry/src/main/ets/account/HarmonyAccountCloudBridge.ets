import http from '@ohos.net.http';
import fs from '@ohos.file.fs';
import common from '@ohos.app.ability.common';
import picker from '@ohos.file.picker';
import util from '@ohos.util';
import client from 'libmsimeclient.so';
import {
  AccountCloudBridge, AccountDownloadResponse, AccountTransportResponse,
  MAX_DICTIONARY_EXPORT_BYTES, MAX_SNAPSHOT_DOWNLOAD_BYTES, dictionaryChangePageChanged,
  accountReplyValue, strictAccountOk
} from './AccountCloudBridge';
import { KeyboardSession } from '../keyboard/KeyboardSession';
import { KeyboardFeedbackBridge } from '../keyboard/KeyboardFeedbackBridge';
import { HarmonyAiSkins } from './HarmonyAiSkins';
import {
  AccountPreferences,
  AccountPreferenceSchema,
  AccountPreferenceSettings,
  AppliedPreferences,
  FeedbackValues,
  applyAccountPreferences,
  accountPreferencesFromDocument,
  localAccountPreferences,
  mergeAccountPreferences,
  preferenceSchemaFromDocument,
  validLocalPreferenceDocument,
} from './AccountPreferencePlan';
import {
  ACCOUNT_ORIGIN, AnonymousSessionStore, FileSessionStore, HarmonyAccountTransport, responseContentLength,
  responseContentType
} from './HarmonyAccountTransport';
import { installNoRedirectGuard, noRedirectOptions } from '../network/HarmonyHttpSecurity';

interface SnapshotAction {
  operation: string;
  token?: string;
}

interface AccountOperation {
  operation?: string;
  dictionary_operation?: string;
  kind?: string;
  format?: string;
}

interface AccountOperationReply {
  ok?: boolean;
}

/** One settings-sync request from the page: read the schema or document, upload, or apply. */
interface PreferenceSyncAction {
  operation: string;
  user_id?: string;
}

interface SnapshotRequest {
  id: string;
  cloudRevision: number;
  expectedLocalVersion: string;
  fileSha256: string;
  status: string;
}

interface NativeSnapshotRequest extends SnapshotRequest {
  accountId: string;
}

interface SnapshotQueueState {
  localVersion?: string;
  request?: NativeSnapshotRequest | null;
}

interface SnapshotQueueReply {
  ok: boolean;
  value?: SnapshotQueueState;
  error?: string;
}

interface SnapshotQueueStateAction {
  operation: string;
  directory: string;
  options: Object;
  acknowledge: boolean;
}

interface SnapshotQueueEnqueueAction {
  operation: string;
  directory: string;
  source: string;
  account_id: string;
  cloud_revision: number;
  expected_local_version: string;
  file_sha256: string;
}

interface SnapshotQueueCancelAction {
  operation: string;
  directory: string;
  account_id: string;
}

interface SnapshotValue {
  version?: string;
  handle?: number;
  localVersion?: string;
  request?: SnapshotRequest | null;
  saved?: boolean;
  snapshot?: SnapshotMetadata;
  previewToken?: string;
  expectedRevision?: number;
  revision?: number;
  reset?: boolean;
}

interface SnapshotReply {
  ok: boolean;
  value: SnapshotValue;
  error?: string;
}

interface DownloadHeaders {
  Accept: string;
  Authorization: string;
  'Accept-Encoding': string;
}

/** The settings application's transport: the shared JSON requests plus the snapshot and export file transfers. */
class HarmonyTransport extends HarmonyAccountTransport {
  private readonly activeDownloads: Set<http.HttpRequest> = new Set<http.HttpRequest>();

  /** Abort large file transfers when the account session is invalidated. */
  cancelDownloads(): void {
    for (const request of this.activeDownloads) request.destroy();
    this.activeDownloads.clear();
  }

  async download(path: string, token: string, destination: string, maximumBytes: number,
    mediaType: string): Promise<AccountDownloadResponse> {
    if (!path.startsWith('/v1/') || path.includes('\\') || destination.length === 0
      || maximumBytes <= 0 || maximumBytes > MAX_SNAPSHOT_DOWNLOAD_BYTES) {
      throw new Error('account_invalid');
    }
    const request = http.createHttp();
    this.activeDownloads.add(request);
    const headers: DownloadHeaders = {
      Accept: mediaType,
      Authorization: `Bearer ${token}`,
      'Accept-Encoding': 'identity'
    };
    const options = noRedirectOptions({
      method: http.RequestMethod.GET,
      header: headers,
      connectTimeout: 30000,
      readTimeout: 600000
    });
    let handle: fs.File | undefined = undefined;
    let bytes: number = 0;
    let failed: boolean = false;
    let contentLength: number | undefined = undefined;
    let contentType: string | undefined = undefined;
    let endResolve: (() => void) | undefined = undefined;
    const ended: Promise<void> = new Promise<void>((resolve: () => void): void => {
      endResolve = resolve;
      // `requestInStream` returns the status code; `dataEnd` is the SDK's explicit statement that
      // the streamed body is complete. Waiting for both keeps the file open through the last chunk.
      request.on('dataEnd', (): void => resolve());
    });
    installNoRedirectGuard(request, (): void => {
      failed = true;
      endResolve?.();
    });
    request.on('headersReceive', (responseHeaders: Object): void => {
      contentLength = responseContentLength(responseHeaders);
      contentType = responseContentType(responseHeaders);
      if (contentLength !== undefined && (contentLength < 0 || contentLength > maximumBytes)) {
        failed = true;
      }
    });
    request.on('dataReceive', (chunk: ArrayBuffer): void => {
      const view: Uint8Array = new Uint8Array(chunk);
      // The file is opened before the request starts streaming, but the closure cannot see that narrowing.
      if (failed || handle === undefined || bytes + view.byteLength > maximumBytes || view.includes(0)) {
        failed = true;
        return;
      }
      try {
        const written: number = fs.writeSync(handle.fd, chunk);
        if (written !== view.byteLength) {
          failed = true;
          return;
        }
        bytes += written;
      } catch {
        failed = true;
      }
    });
    try {
      // Open the destination inside the guarded region: if the picker URI or parent directory is
      // unavailable, the request created above still needs to be destroyed.
      handle = this.openSafeDestination(destination);
      const status: number = await request.requestInStream(`${ACCOUNT_ORIGIN}${path}`, options);
      await ended;
      if (status < 200 || status >= 300) {
        return { status: status, bytes: bytes, contentLength: contentLength, contentType: contentType };
      }
      if (failed || bytes === 0 || contentType !== mediaType
        || (contentLength !== undefined && contentLength !== bytes)) {
        return { status: 503, bytes: bytes, contentLength: contentLength, contentType: contentType };
      }
      return { status: status, bytes: bytes, contentLength: contentLength, contentType: contentType };
    } finally {
      this.activeDownloads.delete(request);
      if (handle !== undefined) fs.closeSync(handle);
      request.destroy();
    }
  }

  async uploadSnapshot(source: string, revision: number, expectedSha256: string,
    token: string): Promise<AccountTransportResponse> {
    try {
      const raw: string = await client.snapshotRestore(JSON.stringify({
        revision: revision, expected_sha256: expectedSha256, access_token: token
      }), source);
      const reply: SnapshotReply = JSON.parse(raw) as SnapshotReply;
      const value: SnapshotValue | null = accountReplyValue(reply);
      if (value !== null) return { status: 200, body: JSON.stringify(value) };
      switch (reply.error) {
        case 'account_invalid': return { status: 400, body: '' };
        case 'account_unauthorized': return { status: 401, body: '' };
        case 'account_conflict': return { status: 409, body: '' };
        case 'account_rate_limited': return { status: 429, body: '' };
        default: return { status: 503, body: '' };
      }
    } catch {
      return { status: 503, body: '' };
    }
  }

  private safeDestination(destination: string): boolean {
    const slash: number = destination.lastIndexOf('/');
    if (slash <= 0) return false;
    const parent: string = destination.substring(0, slash);
    try {
      const parentStat: fs.Stat = fs.lstatSync(parent);
      if (parentStat.isSymbolicLink() || !parentStat.isDirectory()) return false;
      try {
        const targetStat: fs.Stat = fs.lstatSync(destination);
        return !targetStat.isSymbolicLink() && targetStat.isFile();
      } catch {
        return true;
      }
    } catch {
      return false;
    }
  }

  /** Open a private staging file and bind the descriptor to the path checked above. */
  private openSafeDestination(destination: string): fs.File {
    if (!this.safeDestination(destination)) {
      throw new Error('account_storage');
    }
    let handle: fs.File | undefined = undefined;
    try {
      handle = fs.openSync(destination,
        fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      const pathStat: fs.Stat = fs.lstatSync(destination);
      const opened: fs.Stat = fs.statSync(handle.fd);
      if (pathStat.isSymbolicLink() || !pathStat.isFile()
        || !opened.isFile() || opened.ino !== pathStat.ino) {
        throw new Error('account_storage');
      }
      return handle;
    } catch (error) {
      if (handle !== undefined) fs.closeSync(handle);
      // ArkTS 只允许抛 Error（arkts-limited-throw）；原样抛出同一个对象，调用方仍能读到它的错误码。
      throw error as Error;
    }
  }
}

interface SnapshotMetadata {
  cloudRevision: number;
  sha256: string;
  fileSha256: string;
  bytes: number;
  records: number;
  entries: number;
  overlays: number;
  positions: number;
  selections: number;
  engineRecords: number;
}

interface SnapshotInspectReply {
  ok: boolean;
  value?: SnapshotMetadata;
  error?: string;
}

/** Account and cloud clipboard state stays in the host; WebView receives only bounded DTOs. */
export class HarmonyAccountCloudBridge {
  private readonly transport: HarmonyTransport;
  private readonly bridge: AccountCloudBridge;
  private readonly context: common.UIAbilityContext;
  private snapshotFile: string = '';
  private snapshotToken: string = '';
  private snapshotAccountId: string = '';
  private snapshotExpectedVersion: string = '';
  private snapshotMetadata: SnapshotMetadata | undefined = undefined;
  private restoreFile: string = '';
  private restoreToken: string = '';
  private restoreAccountId: string = '';
  private restoreMetadata: SnapshotMetadata | undefined = undefined;
  private restoreExpectedRevision: number = -1;

  constructor(context: common.UIAbilityContext) {
    this.context = context;
    const directory = `${context.filesDir}/state`;
    try {
      if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);
    } catch {
      // The store reports an unavailable session if the sandbox cannot be prepared.
    }
    this.transport = new HarmonyTransport();
    // 匿名会话就是 EntryAbility 在同一目录下注册的那个；无人登录时反馈回退到它，与 Android 一致。
    this.bridge = new AccountCloudBridge(this.transport, new FileSessionStore(directory),
      new AnonymousSessionStore(directory));
  }

  async request(action: string): Promise<string> {
    const accountId: string | null = this.bridge.currentUserId();
    let operation: string = '';
    let decoded: AccountOperation | undefined = undefined;
    try {
      decoded = JSON.parse(action) as AccountOperation;
      operation = typeof decoded.operation === 'string' ? decoded.operation : '';
    } catch {
      // The account bridge reports malformed action documents.
    }
    if (operation === 'logout' || operation === 'delete_account'
      || operation === 'login' || operation === 'clear_expired') {
      this.transport.cancelDownloads();
      this.clearSnapshotPreview();
      this.clearRestorePreview();
      if (accountId !== null && !this.cancelQueueForAccount(accountId)) {
        return this.failure('snapshot_unavailable');
      }
    }
    if (decoded !== undefined && operation === 'dictionary'
      && decoded.dictionary_operation === 'export') {
      return this.exportDictionary(decoded);
    }
    const result = await this.bridge.handle(action);
    if (accountId === null || this.bridge.currentUserId() === accountId) return result;
    try {
      const reply: AccountOperationReply = JSON.parse(result) as AccountOperationReply;
      if (operation.length > 0 && reply.ok === true) {
        this.cancelQueueForAccount(accountId);
      }
    } catch {
      // The account bridge already owns malformed request and response reporting.
    }
    return result;
  }

  /** Download and validate privately before asking the user where the finished export should go. */
  private async exportDictionary(action: AccountOperation): Promise<string> {
    const kind: string = typeof action.kind === 'string' ? action.kind : '';
    const format: string = typeof action.format === 'string' ? action.format : '';
    // `wubi98` 是 98 五笔的云端词库，路径与种类同名。
    if (!['pinyin', 'wubi', 'wubi98', 'quick', 'english'].includes(kind)
      || !['standard', 'windows'].includes(format)) {
      return this.failure('account_invalid');
    }
    const directory: string = `${this.context.filesDir}/state/dictionary-exports`;
    const staging: string = `${directory}/${util.generateRandomUUID(false)}.tsv`;
    let destination: string = '';
    try {
      if (!this.safeDirectory(`${this.context.filesDir}/state`)) {
        return this.failure('account_storage');
      }
      if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);
      if (!this.safeDirectory(directory)) {
        return this.failure('account_storage');
      }
      const downloaded = await this.bridge.downloadAuthenticated(
        `/v1/users/me/dictionaries/${kind}/export?format=${format}`,
        staging, MAX_DICTIONARY_EXPORT_BYTES, 'text/plain');
      if (downloaded.bytes === undefined) {
        return this.failure(downloaded.error ?? 'account_unavailable');
      }
      const options: picker.DocumentSaveOptions = new picker.DocumentSaveOptions();
      options.newFileNames = [`msime-${kind}-dictionary.tsv`];
      options.fileSuffixChoices = ['TSV|.tsv'];
      const selected: string[] = await new picker.DocumentViewPicker(this.context).save(options);
      if (selected.length === 0) return this.success({ saved: false });
      destination = selected[0];
      await fs.copyFile(staging, destination);
      return this.success({ saved: true });
    } catch {
      try {
        if (destination.length > 0 && fs.accessSync(destination)) fs.unlinkSync(destination);
      } catch {
        // Refusing the export matters more than a picker URI which has already disappeared.
      }
      return this.failure('account_storage');
    } finally {
      try {
        if (fs.accessSync(staging)) fs.unlinkSync(staging);
      } catch {
        // A missing staging file is already the desired postcondition.
      }
    }
  }

  private cancelQueueForAccount(accountId: string): boolean {
    try {
      const action: SnapshotQueueCancelAction = {
        operation: 'cancel', directory: `${this.context.filesDir}/state/dictionary-snapshots`,
        account_id: accountId
      };
      const reply: SnapshotQueueReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueReply;
      return strictAccountOk(reply.ok);
    } catch {
      return false;
    }
  }

  /**
   * The AI skin runner, which shares this bridge's session rather than holding one of its own.
   *
   * Its four requests are authenticated as the same user the rest of the account surface is, and a
   * sign-out has to stop a run that is in flight. A second session object would have missed that.
   */
  aiSkins(): HarmonyAiSkins {
    return new HarmonyAiSkins(this.bridge);
  }

  /**
   * The settings sync, which is the only account surface that writes to the device.
   *
   * It is kept here rather than in `AccountCloudBridge` because the mapping needs two things that
   * bridge has no business holding: the local preference document, which only the shared NAPI can
   * read and write, and the key-feedback file, which lives beside it. The bridge stays what it is —
   * a session and a transport — and the decisions about which settings are account settings live in
   * `AccountPreferencePlan`, where they can be tested without either.
   */
  async preferencesSync(actionDocument: string, stateRoot: string): Promise<string> {
    let action: PreferenceSyncAction;
    try {
      action = JSON.parse(actionDocument) as PreferenceSyncAction;
      if (typeof action.operation !== 'string') return this.failure('account_invalid');
    } catch {
      return this.failure('account_invalid');
    }
    try {
      switch (action.operation) {
        case 'preferences_schema':
          return await this.preferenceSchema();
        case 'preferences_load':
          return await this.preferenceDocument();
        case 'preferences_upload':
          return await this.uploadPreferences(stateRoot);
        case 'preferences_apply':
          return await this.applyPreferences(String(action.user_id ?? ''), stateRoot);
        default:
          return this.failure('account_invalid');
      }
    } catch (error) {
      return this.failure(error instanceof Error ? error.message : 'account_unavailable');
    }
  }

  /**
   * The schema, renamed to what the page reads.
   *
   * The service answers in snake_case and the shared settings surface expects camelCase — the Tauri
   * hosts rename it on the way through, and a host that forwarded the wire shape would leave the
   * page reading `undefined` for the byte limit and the update mode, which it then treats as a
   * schema it cannot trust.
   */
  private async preferenceSchema(): Promise<string> {
    const reply = await this.bridge.preferenceSchema();
    if (reply.error !== undefined || reply.value === undefined || reply.value === null) {
      return this.failure(reply.error ?? 'account_unavailable');
    }
    const schema = this.schemaOf(reply.value);
    return schema === null ? this.failure('account_unavailable')
      : JSON.stringify({ ok: true, value: schema });
  }

  private schemaOf(document: Record<string, Object>): AccountPreferenceSchema | null {
    return preferenceSchemaFromDocument(document);
  }

  private async preferenceDocument(): Promise<string> {
    const reply = await this.bridge.loadPreferenceDocument();
    if (reply.error !== undefined || reply.value === undefined || reply.value === null) {
      return this.failure(reply.error ?? 'account_unavailable');
    }
    const cloud = this.cloudOf(reply.value);
    return cloud === null ? this.failure('account_unavailable')
      : JSON.stringify({ ok: true, value: cloud });
  }

  private cloudOf(document: Record<string, Object>): AccountPreferences | null {
    return accountPreferencesFromDocument(document);
  }

  /** What the local document says, read the same way the settings page reads it. */
  private localDocument(stateRoot: string): Record<string, Object> | null {
    try {
      const reply: Record<string, Object> =
        JSON.parse(client.loadPreferences(stateRoot)) as Record<string, Object>;
      if (reply['ok'] !== true) return null;
      const value: Record<string, Object> = reply['value'] as Record<string, Object>;
      return validLocalPreferenceDocument(value) ? value : null;
    } catch {
      return null;
    }
  }

  private feedbackValues(stateRoot: string): FeedbackValues {
    const shared = KeyboardFeedbackBridge.toShared(KeyboardSession.feedbackIn(stateRoot));
    return {
      soundEnabled: shared.soundEnabled,
      hapticsEnabled: shared.hapticsEnabled,
      hapticStrength: shared.hapticStrength
    };
  }

  /**
   * This device's settings, written over what the account already holds.
   *
   * The cloud document is read first and merged into rather than replaced: it carries the fields of
   * every other device the user has signed in from, and an upload from a phone has no business
   * clearing what a desktop wrote. Keys the schema has not declared are dropped here rather than
   * sent, because the service rejects the whole write for one unknown key — which would make this
   * button fail entirely for the sake of a setting that simply has nowhere to go yet.
   */
  private async uploadPreferences(stateRoot: string): Promise<string> {
    const expectedGeneration: number = this.bridge.sessionGeneration();
    const schemaReply = await this.bridge.preferenceSchema();
    if (schemaReply.error !== undefined || schemaReply.value === undefined || schemaReply.value === null) {
      return this.failure(schemaReply.error ?? 'account_unavailable');
    }
    const schema = this.schemaOf(schemaReply.value);
    if (schema === null) return this.failure('account_unavailable');
    const cloudReply = await this.bridge.loadPreferenceDocument();
    if (cloudReply.error !== undefined || cloudReply.value === undefined || cloudReply.value === null) {
      return this.failure(cloudReply.error ?? 'account_unavailable');
    }
    const cloud = this.cloudOf(cloudReply.value);
    if (cloud === null) return this.failure('account_unavailable');
    const local = this.localDocument(stateRoot);
    if (local === null) return this.failure('account_storage');
    const values = localAccountPreferences(
      local['preferences'] as Record<string, Object>, this.feedbackValues(stateRoot));
    const declared: AccountPreferenceSettings = {};
    let declaredAny = false;
    for (const key of Object.keys(values)) {
      if (schema.fields[key] !== undefined) {
        declared[key] = values[key];
        declaredAny = true;
      }
    }
    if (!declaredAny) return this.failure('account_unavailable');
    const merged = mergeAccountPreferences(cloud, declared, schema);
    if (this.bridge.sessionGeneration() !== expectedGeneration) {
      return this.failure('account_cancelled');
    }
    const putReply = await this.bridge.putPreferenceDocument(
      merged as Object as Record<string, Object>);
    if (putReply.error !== undefined || putReply.value === undefined || putReply.value === null) {
      return this.failure(putReply.error ?? 'account_unavailable');
    }
    const updated = this.cloudOf(putReply.value);
    return updated === null ? this.failure('account_unavailable')
      : JSON.stringify({ ok: true, value: updated });
  }

  /**
   * The account's settings, written onto this device.
   *
   * The user id the page names is checked against the session first. It is the one guard this
   * direction needs: the page read those values under one account, and if a different one is signed
   * in by the time the confirmation is answered, writing them replaces the user's own settings with
   * a stranger's and nothing on this screen can put them back.
   */
  private async applyPreferences(userId: string, stateRoot: string): Promise<string> {
    const expectedGeneration: number = this.bridge.sessionGeneration();
    if (userId.length === 0 || this.bridge.currentUserId() !== userId) {
      return this.failure('account_unauthorized');
    }
    const schemaReply = await this.bridge.preferenceSchema();
    if (schemaReply.error !== undefined || schemaReply.value === undefined || schemaReply.value === null) {
      return this.failure(schemaReply.error ?? 'account_unavailable');
    }
    const schema = this.schemaOf(schemaReply.value);
    if (schema === null) return this.failure('account_unavailable');
    const cloudReply = await this.bridge.loadPreferenceDocument();
    if (cloudReply.error !== undefined || cloudReply.value === undefined || cloudReply.value === null) {
      return this.failure(cloudReply.error ?? 'account_unavailable');
    }
    const cloud = this.cloudOf(cloudReply.value);
    if (cloud === null) return this.failure('account_unavailable');
    // Re-checked after the round trip: a sign-out during it invalidates the answer that just came
    // back, and this is the last point before the write where saying so still costs nothing.
    if (this.bridge.sessionGeneration() !== expectedGeneration
      || this.bridge.currentUserId() !== userId) return this.failure('account_cancelled');
    const local = this.localDocument(stateRoot);
    if (local === null) return this.failure('account_storage');
    const applied: AppliedPreferences = applyAccountPreferences(
      local['preferences'] as Record<string, Object>, cloud, schema,
      this.feedbackValues(stateRoot));
    const revision: number = local['revision'] as number;
    const saved: string = client.savePreferences(stateRoot, revision,
      JSON.stringify(applied.preferences));
    try {
      const reply: Record<string, Object> = JSON.parse(saved) as Record<string, Object>;
      if (reply['ok'] !== true) return this.failure('account_storage');
    } catch {
      return this.failure('account_storage');
    }
    // The feedback file is written after the document, and its failure is reported rather than
    // rolled back: the settings that did land are correct, and a second attempt is harmless.
    if (applied.feedback !== null) {
      // 滑行输入和手写设置与账号共用文件但不随账号同步，所以保留这台设备上的值：账号不携带的字段都由已存储的设置填充。
      const written = KeyboardSession.saveFeedbackIn(stateRoot,
        KeyboardFeedbackBridge.fromShared({
          soundEnabled: applied.feedback.soundEnabled,
          hapticsEnabled: applied.feedback.hapticsEnabled,
          hapticStrength: applied.feedback.hapticStrength
        }, KeyboardSession.feedbackIn(stateRoot)));
      if (!written) return this.failure('account_storage');
    }
    return this.success({});
  }

  async snapshot(actionDocument: string, resources: string, stateRoot: string): Promise<string> {
    let action: SnapshotAction;
    try {
      action = JSON.parse(actionDocument) as SnapshotAction;
      if (typeof action.operation !== 'string') return this.failure('snapshot_invalid');
    } catch {
      return this.failure('snapshot_invalid');
    }
    switch (action.operation) {
      case 'snapshot_status':
        return this.status(resources, stateRoot);
      case 'snapshot_export':
        return this.exportSnapshot(resources, stateRoot);
      case 'snapshot_preview':
        return this.preview(resources, stateRoot);
      case 'snapshot_restore_preview':
        return this.restorePreview(stateRoot);
      case 'snapshot_restore_native':
        return this.restoreNative(String(action.token ?? ''));
      case 'snapshot_restore_cancel':
        this.clearRestorePreview();
        return this.success({});
      case 'snapshot_enqueue':
        return this.enqueue(String(action.token ?? ''), stateRoot);
      case 'snapshot_cancel':
        return this.cancel(stateRoot);
      default:
        return this.failure('snapshot_unavailable');
    }
  }

  private failure(code: string): string {
    return JSON.stringify({ ok: false, error: code });
  }

  private success(value: SnapshotValue): string {
    return JSON.stringify({ ok: true, value: value });
  }

  private options(resources: string, stateRoot: string): Object | null {
    try {
      const reply: SnapshotReply = JSON.parse(client.prepareHost(JSON.stringify(
        KeyboardSession.prepareRequest(resources, stateRoot)))) as SnapshotReply;
      return accountReplyValue(reply);
    } catch {
      return null;
    }
  }

  private queueDirectory(stateRoot: string): string {
    return `${stateRoot}/dictionary-snapshots`;
  }

  private queueState(resources: string, stateRoot: string,
    acknowledge: boolean): SnapshotQueueState | null {
    const options = this.options(resources, stateRoot);
    if (options === null) return null;
    try {
      const action: SnapshotQueueStateAction = {
        operation: 'state', directory: this.queueDirectory(stateRoot),
        options: options, acknowledge: acknowledge
      };
      const reply: SnapshotQueueReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueReply;
      return accountReplyValue(reply);
    } catch {
      return null;
    }
  }

  private publicRequest(request: NativeSnapshotRequest | null | undefined): SnapshotRequest | null {
    if (request === undefined || request === null) return null;
    return {
      id: request.id,
      cloudRevision: request.cloudRevision,
      expectedLocalVersion: request.expectedLocalVersion,
      fileSha256: request.fileSha256,
      status: request.status
    };
  }

  private async status(resources: string, stateRoot: string): Promise<string> {
    const state = this.queueState(resources, stateRoot, true);
    if (state === null || typeof state.localVersion !== 'string') {
      return this.failure('snapshot_unavailable');
    }
    return this.success({
      localVersion: state.localVersion,
      request: this.publicRequest(state.request)
    });
  }

  private async exportSnapshot(resources: string, stateRoot: string): Promise<string> {
    const result = await this.downloadSnapshot(stateRoot);
    if (result === null) return this.failure('snapshot_unavailable');
    let destination: string = '';
    try {
      const options: picker.DocumentSaveOptions = new picker.DocumentSaveOptions();
      options.newFileNames = ['msime-dictionary-snapshot.ndjson'];
      options.fileSuffixChoices = ['NDJSON|.ndjson'];
      const selected: string[] = await new picker.DocumentViewPicker(this.context).save(options);
      if (selected.length === 0) return this.success({ saved: false, snapshot: result });
      destination = selected[0];
      await fs.copyFile(this.snapshotFile, destination);
      return this.success({ saved: true, snapshot: result });
    } catch {
      try {
        if (destination.length > 0 && fs.accessSync(destination)) fs.unlinkSync(destination);
      } catch {
        // The selected URI may disappear with the picker; no partial export is still the goal.
      }
      return this.failure('account_storage');
    } finally {
      this.clearSnapshotPreview();
    }
  }

  private async preview(resources: string, stateRoot: string): Promise<string> {
    const accountId: string | null = this.bridge.currentUserId();
    if (accountId === null) return this.failure('account_unauthorized');
    const result = await this.downloadSnapshot(stateRoot);
    const state = this.queueState(resources, stateRoot, false);
    if (result === null || state === null || typeof state.localVersion !== 'string') {
      this.clearSnapshotPreview();
      return this.failure('snapshot_unavailable');
    }
    if (this.bridge.currentUserId() !== accountId) {
      this.clearSnapshotPreview();
      return this.failure('account_cancelled');
    }
    this.snapshotAccountId = accountId;
    this.snapshotExpectedVersion = state.localVersion;
    this.snapshotToken = util.generateRandomUUID(false);
    return this.success({ previewToken: this.snapshotToken, snapshot: result });
  }

  private inspectSnapshot(file: string): SnapshotMetadata | null {
    try {
      const reply: SnapshotInspectReply = JSON.parse(client.snapshotInspect(file)) as SnapshotInspectReply;
      return accountReplyValue(reply);
    } catch {
      return null;
    }
  }

  private clearSnapshotPreview(): void {
    try {
      if (this.snapshotFile.length > 0 && fs.accessSync(this.snapshotFile)) fs.unlinkSync(this.snapshotFile);
    } catch {
      // A failed or already absent download has nothing left to remove.
    }
    this.snapshotToken = '';
    this.snapshotAccountId = '';
    this.snapshotFile = '';
    this.snapshotExpectedVersion = '';
    this.snapshotMetadata = undefined;
  }

  private clearRestorePreview(): void {
    try {
      if (this.restoreFile.length > 0 && fs.accessSync(this.restoreFile)) {
        fs.unlinkSync(this.restoreFile);
      }
    } catch {
      // A missing private copy is already the desired postcondition.
    }
    this.restoreFile = '';
    this.restoreToken = '';
    this.restoreAccountId = '';
    this.restoreMetadata = undefined;
    this.restoreExpectedRevision = -1;
  }

  private async restorePreview(stateRoot: string): Promise<string> {
    const accountId: string | null = this.bridge.currentUserId();
    if (accountId === null) return this.failure('account_unauthorized');
    if (!this.safeDirectory(stateRoot)) return this.failure('account_storage');
    this.clearRestorePreview();
    try {
      const options: picker.DocumentSelectOptions = new picker.DocumentSelectOptions();
      options.maxSelectNumber = 1;
      options.fileSuffixFilters = ['NDJSON|.ndjson'];
      const selected: string[] = await new picker.DocumentViewPicker(this.context).select(options);
      if (selected.length === 0) return this.success({ saved: false });
      if (this.bridge.currentUserId() !== accountId) return this.failure('account_cancelled');
      const source: string = selected[0];
      const sourceFile: fs.File = fs.openSync(source, fs.OpenMode.READ_ONLY);
      let copied: boolean = false;
      let copiedBytes: number = 0;
      try {
        const sourcePathStat: fs.Stat = fs.lstatSync(source);
        const sourceFdStat: fs.Stat = fs.statSync(sourceFile.fd);
        if (sourcePathStat.isSymbolicLink() || !sourcePathStat.isFile()
          || !sourceFdStat.isFile() || sourceFdStat.ino !== sourcePathStat.ino) {
          return this.failure('snapshot_invalid');
        }
        const sourceBytes: number = sourceFdStat.size;
        if (sourceBytes <= 0 || sourceBytes > MAX_SNAPSHOT_DOWNLOAD_BYTES) {
          return this.failure('snapshot_invalid');
        }
        const directory: string = `${stateRoot}/cloud-restore`;
        if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);
        if (!this.safeDirectory(directory)) {
          return this.failure('account_storage');
        }
        this.restoreFile = `${directory}/${util.generateRandomUUID(false)}.ndjson`;
        await fs.copyFile(sourceFile.fd, this.restoreFile);
        copied = true;
        copiedBytes = fs.statSync(this.restoreFile).size;
        if (copiedBytes !== sourceBytes || copiedBytes <= 0
          || copiedBytes > MAX_SNAPSHOT_DOWNLOAD_BYTES) {
          this.clearRestorePreview();
          return this.failure('snapshot_invalid');
        }
      } finally {
        fs.closeSync(sourceFile);
      }
      if (!copied) return this.failure('snapshot_invalid');
      const metadata: SnapshotMetadata | null = this.inspectSnapshot(this.restoreFile);
      if (metadata === null || metadata.bytes !== copiedBytes) {
        this.clearRestorePreview();
        return this.failure('snapshot_invalid');
      }
      const current = await this.bridge.currentDictionaryRevision();
      if (current.revision === undefined) {
        this.clearRestorePreview();
        return this.failure(current.error ?? 'account_unavailable');
      }
      if (this.bridge.currentUserId() !== accountId) {
        this.clearRestorePreview();
        return this.failure('account_cancelled');
      }
      this.restoreAccountId = accountId;
      this.restoreMetadata = metadata;
      this.restoreExpectedRevision = current.revision;
      this.restoreToken = util.generateRandomUUID(false);
      return this.success({
        saved: true, previewToken: this.restoreToken, snapshot: metadata,
        expectedRevision: this.restoreExpectedRevision
      });
    } catch {
      this.clearRestorePreview();
      return this.failure('snapshot_unavailable');
    }
  }

  private async restoreNative(token: string): Promise<string> {
    if (token.length === 0 || token !== this.restoreToken || this.restoreFile.length === 0
      || this.restoreMetadata === undefined || this.restoreExpectedRevision < 0
      || this.restoreAccountId.length === 0) {
      return this.failure('snapshot_invalid');
    }
    if (this.bridge.currentUserId() !== this.restoreAccountId) {
      this.clearRestorePreview();
      return this.failure('account_cancelled');
    }
    const file: string = this.restoreFile;
    const revision: number = this.restoreExpectedRevision;
    const sha256: string = this.restoreMetadata.fileSha256;
    try {
      const restored = await this.bridge.restoreSnapshotAuthenticated(file, revision, sha256);
      if (restored.value === undefined || restored.value === null) {
        return this.failure(restored.error ?? 'account_unavailable');
      }
      return this.success(restored.value as SnapshotValue);
    } catch {
      return this.failure('account_unavailable');
    } finally {
      this.clearRestorePreview();
    }
  }

  private async downloadSnapshot(stateRoot: string): Promise<SnapshotMetadata | null> {
    try {
      this.clearSnapshotPreview();
      if (!this.safeDirectory(stateRoot)) return null;
      const directory = `${stateRoot}/cloud-snapshot`;
      if (!fs.accessSync(directory)) fs.mkdirSync(directory, true);
      this.snapshotFile = `${directory}/snapshot.ndjson`;
      const downloaded = await this.bridge.downloadAuthenticated(
        '/v1/users/me/dictionary/snapshot', this.snapshotFile,
        MAX_SNAPSHOT_DOWNLOAD_BYTES, 'application/x-ndjson');
      if (downloaded.bytes === undefined) {
        this.clearSnapshotPreview();
        return null;
      }
      const metadata = this.inspectSnapshot(this.snapshotFile);
      if (metadata === null) {
        this.clearSnapshotPreview();
        return null;
      }
      this.snapshotMetadata = metadata;
      return this.snapshotMetadata;
    } catch {
      this.clearSnapshotPreview();
      return null;
    }
  }

  private async enqueue(token: string, stateRoot: string): Promise<string> {
    if (token.length === 0 || token !== this.snapshotToken || this.snapshotFile.length === 0
      || this.snapshotMetadata === undefined || this.snapshotExpectedVersion.length === 0
      || this.snapshotAccountId.length === 0
      || this.bridge.currentUserId() !== this.snapshotAccountId) {
      return this.failure('snapshot_invalid');
    }
    try {
      const changes = await this.bridge.rawAuthenticated('GET',
        `/v1/users/me/dictionary/changes?after=${this.snapshotMetadata.cloudRevision}&limit=1`);
      if (changes.status < 200 || changes.status >= 300 || changes.body.length === 0) {
        return this.failure('snapshot_unavailable');
      }
      const changed: boolean | null = dictionaryChangePageChanged(
        changes.body, this.snapshotMetadata.cloudRevision);
      if (changed === null) return this.failure('snapshot_unavailable');
      if (changed || token !== this.snapshotToken
        || this.snapshotMetadata === undefined || this.snapshotFile.length === 0
        || this.bridge.currentUserId() !== this.snapshotAccountId) {
        this.clearSnapshotPreview();
        return this.failure('snapshot_conflict');
      }
      const inspected = this.inspectSnapshot(this.snapshotFile);
      if (inspected === null || inspected.fileSha256 !== this.snapshotMetadata.fileSha256
        || inspected.sha256 !== this.snapshotMetadata.sha256
        || inspected.engineRecords !== this.snapshotMetadata.engineRecords) {
        this.clearSnapshotPreview();
        return this.failure('snapshot_invalid');
      }
      const action: SnapshotQueueEnqueueAction = {
        operation: 'enqueue', directory: this.queueDirectory(stateRoot), source: this.snapshotFile,
        account_id: this.snapshotAccountId, cloud_revision: this.snapshotMetadata.cloudRevision,
        expected_local_version: this.snapshotExpectedVersion,
        file_sha256: this.snapshotMetadata.fileSha256
      };
      const reply: SnapshotQueueReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueReply;
      const queueValue: SnapshotQueueState | null = accountReplyValue(reply);
      if (queueValue === null) {
        return this.failure(reply.error ?? 'snapshot_unavailable');
      }
      const request: SnapshotRequest | null = this.publicRequest(queueValue.request);
      this.clearSnapshotPreview();
      return this.success({ request: request });
    } catch {
      return this.failure('snapshot_unavailable');
    }
  }

  private cancel(stateRoot: string): string {
    const accountId: string | null = this.bridge.currentUserId();
    if (accountId === null) return this.failure('account_unauthorized');
    try {
      const action: SnapshotQueueCancelAction = {
        operation: 'cancel', directory: this.queueDirectory(stateRoot), account_id: accountId
      };
      const reply: SnapshotQueueReply =
        JSON.parse(client.snapshotQueue(JSON.stringify(action))) as SnapshotQueueReply;
      const queueValue: SnapshotQueueState | null = accountReplyValue(reply);
      if (queueValue === null) {
        return this.failure(reply.error ?? 'snapshot_unavailable');
      }
      this.clearSnapshotPreview();
      return this.success({ request: this.publicRequest(queueValue.request) });
    } catch {
      return this.failure('snapshot_unavailable');
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
}
