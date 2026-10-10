import http from '@ohos.net.http';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import client from 'libmsimeclient.so';
import {
  AccountSessionStore, AccountTransport, AccountTransportResponse, parseResponseContentLength,
  strictAccountOk
} from './AccountCloudBridge';
import { installNoRedirectGuard, noRedirectOptions } from '../network/HarmonyHttpSecurity';
import { KeyboardLog } from '../keyboard/KeyboardLog';
import {
  anonymousSessionForBridge, anonymousSessionForStorage, MAX_SESSION_BYTES, sessionFitsStorage, sessionWriteComplete
} from './AccountSessionPolicy';

/**
 * The account session file and the HTTPS transport, shared by the settings application and the keyboard extension.
 *
 * Both processes belong to the one `entry` module, so `<files>/state` is the same sandbox directory in each, and the keyboard reads the session the settings page signed in with rather than holding a copy of its own. Nothing here is exported to a WebView.
 */
export const ACCOUNT_ORIGIN: string = 'https://api.msime.app';

interface SnapshotCancelReply {
  ok: boolean;
}

export function cancelSnapshotQueueForAccount(directory: string, accountId: string): boolean {
  try {
    const reply: SnapshotCancelReply = JSON.parse(client.snapshotQueue(JSON.stringify({
      operation: 'cancel', directory: `${directory}/dictionary-snapshots`, account_id: accountId
    }))) as SnapshotCancelReply;
    return reply !== null && strictAccountOk(reply.ok);
  } catch {
    return false;
  }
}

/**
 * `<state>/account-session.json`, written whole and then renamed into place.
 *
 * Two processes read this file. `AccountCloudBridge` refuses and clears a document it cannot parse, so a reader that caught a truncate-then-write halfway would sign the user out; writing a uniquely named sibling and renaming it means a reader only ever sees the old document or the new one.
 */
export class FileSessionStore implements AccountSessionStore {
  private readonly directory: string;
  private readonly file: string;
  private readonly lockFile: string;
  private readonly cancelSnapshots: boolean;

  /** `name` 是文件名主干：已登录账号为 `account-session`，设备匿名账号为 `anonymous-session`。 */
  constructor(directory: string, name: string = 'account-session') {
    this.directory = directory;
    this.file = `${directory}/${name}.json`;
    this.lockFile = `${directory}/${name}.lock`;
    this.cancelSnapshots = name === 'account-session';
  }

  /** Queue cancellation shares a lock with activation, so an invalidated owner cannot apply a snapshot after this returns. */
  beforeClear(accountId: string): void {
    if (this.cancelSnapshots && !cancelSnapshotQueueForAccount(this.directory, accountId)) {
      throw new Error('snapshot_unavailable');
    }
  }

  /**
   * Runs `body` under an exclusive advisory lock on `<state>/account-session.lock`.
   *
   * The service rotates refresh tokens and revokes the session when a spent one is presented, so the settings application and the keyboard extension must never refresh at the same time or from a stale read. Every refresh in either process happens inside this lock, after `AccountCloudBridge` has re-read the session file. The lock is per open file, so two bridges in one process exclude each other too, and the kernel releases it if the holder dies.
   */
  async exclusive<T>(body: () => Promise<T>): Promise<T> {
    if (!this.prepareDirectory() || this.isSymbolicLink(this.lockFile)) {
      throw new Error('account_storage');
    }
    const lock: fs.File = fs.openSync(this.lockFile, fs.OpenMode.CREATE | fs.OpenMode.READ_WRITE);
    try {
      const pathStat: fs.Stat = fs.lstatSync(this.lockFile);
      const opened: fs.Stat = fs.statSync(lock.fd);
      if (pathStat.isSymbolicLink() || !pathStat.isFile()
        || !opened.isFile() || opened.ino !== pathStat.ino) {
        throw new Error('account_storage');
      }
      await lock.lock(true);
      try {
        return await body();
      } finally {
        lock.unlock();
      }
    } finally {
      fs.closeSync(lock);
    }
  }

  load(): string | null {
    let handle: fs.File | null = null;
    try {
      if (!this.safeDirectory()) return null;
      const expected: fs.Stat = fs.lstatSync(this.file);
      if (expected.isSymbolicLink() || !expected.isFile() || expected.size > MAX_SESSION_BYTES) return null;
      handle = fs.openSync(this.file, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > MAX_SESSION_BYTES || stat.ino !== expected.ino) return null;
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) return null;
      return util.TextDecoder.create('utf-8').decode(new Uint8Array(buffer, 0, count));
    } catch {
      return null;
    } finally {
      if (handle !== null) fs.closeSync(handle);
    }
  }

  save(value: string): void {
    if (!sessionFitsStorage(value) || !this.prepareDirectory() || this.isSymbolicLink(this.file)) {
      throw new Error('account_storage');
    }
    const staging: string = `${this.file}.${util.generateRandomUUID(false)}.tmp`;
    let renamed: boolean = false;
    try {
      const handle = fs.openSync(staging, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        const written: number = fs.writeSync(handle.fd, value);
        if (!sessionWriteComplete(value, written)) throw new Error('account_storage');
      } finally {
        fs.closeSync(handle);
      }
      fs.renameSync(staging, this.file);
      renamed = true;
    } finally {
      if (!renamed) this.discard(staging);
    }
  }

  private discard(staging: string): void {
    try {
      if (fs.accessSync(staging)) fs.unlinkSync(staging);
    } catch {
      // The staging file is private and uniquely named; a leftover one is never read.
    }
  }

  clear(): void {
    try {
      if (!this.safeDirectory() || this.isSymbolicLink(this.file)) return;
      if (fs.accessSync(this.file)) fs.unlinkSync(this.file);
    } catch {
      // Clearing an already absent session is idempotent.
    }
  }

  private prepareDirectory(): boolean {
    try {
      if (!fs.accessSync(this.directory)) fs.mkdirSync(this.directory, true);
      return this.safeDirectory();
    } catch {
      return false;
    }
  }

  private safeDirectory(): boolean {
    try {
      const stat: fs.Stat = fs.lstatSync(this.directory);
      return !stat.isSymbolicLink() && stat.isDirectory();
    } catch {
      return false;
    }
  }

  private isSymbolicLink(path: string): boolean {
    try {
      return fs.lstatSync(path).isSymbolicLink();
    } catch {
      return false;
    }
  }
}

/**
 * `<state>/anonymous-session.json`：原生客户端首次启动时注册的设备匿名账号（`ensureAnonymousAccount`），作为 `AccountCloudBridge` 的存储。
 *
 * Android 上优先带登录账号、否则带匿名账号发送的请求（应用内反馈），在无人登录时使用它。文件保持其他客户端共用的原生格式，读写时转换，因此在这里保存的轮换结果仍是原生客户端能找到的会话；锁归本存储自己所有，因为只有 bridge 会刷新这个会话。
 */
export class AnonymousSessionStore implements AccountSessionStore {
  private readonly file: FileSessionStore;

  constructor(directory: string) {
    this.file = new FileSessionStore(directory, 'anonymous-session');
  }

  async exclusive<T>(body: () => Promise<T>): Promise<T> {
    return await this.file.exclusive<T>(body);
  }

  load(): string | null {
    const saved: string | null = this.file.load();
    return saved === null ? null : anonymousSessionForBridge(saved);
  }

  save(value: string): void {
    const saved: string | null = anonymousSessionForStorage(value, Date.now());
    if (saved === null) {
      throw new Error('account_storage');
    }
    this.file.save(saved);
  }

  clear(): void {
    this.file.clear();
  }
}

interface HttpHeaders {
  Accept: string;
  Authorization?: string;
  'Content-Type': string;
}

export function responseHeader(headers: Object, name: string): string | undefined {
  const values: Record<string, Object> = headers as Record<string, Object>;
  const keys: string[] = Object.keys(values);
  for (const key of keys) {
    if (key.toLowerCase() !== name) continue;
    const raw: Object = values[key];
    return typeof raw === 'string' ? raw : undefined;
  }
  return undefined;
}

/** A declared response size lets native downloads reject a connection that ended early. */
export function responseContentLength(headers: Object): number | undefined {
  const raw: string | undefined = responseHeader(headers, 'content-length');
  if (raw === undefined) return undefined;
  return parseResponseContentLength(raw);
}

export function responseContentType(headers: Object): string | undefined {
  const raw: string | undefined = responseHeader(headers, 'content-type');
  return raw === undefined ? undefined : raw.split(';')[0].trim().toLowerCase();
}

/** JSON requests to the account service through the system HTTPS stack; the settings application adds file transfers on top. */
/** 后端错误正文 `{"error":{"code":"..."}}` 里的码；不是这个形状、或码不像标识符时为 `-`。 */
function backendErrorCode(result: string | Object | ArrayBuffer): string {
  if (typeof result !== 'string' || result.length > 4096) return '-';
  try {
    const parsed: BackendError = JSON.parse(result) as BackendError;
    const code: string | undefined = parsed.error?.code;
    return typeof code === 'string' && /^[a-z_]{1,64}$/.test(code) ? code : '-';
  } catch {
    return '-';
  }
}

interface BackendError {
  error?: BackendErrorBody;
}

interface BackendErrorBody {
  code?: string;
}

export class HarmonyAccountTransport implements AccountTransport {
  private readonly activeRequests: Map<string, Set<http.HttpRequest>> =
    new Map<string, Set<http.HttpRequest>>();

  async request(method: string, path: string, token?: string, body?: Object,
    timeoutMs?: number, requestTag?: string): Promise<AccountTransportResponse> {
    if (!path.startsWith('/v1/') || path.includes('\\')) throw new Error('account_invalid');
    const request = http.createHttp();
    installNoRedirectGuard(request);
    if (requestTag !== undefined && requestTag.length > 0) {
      let requests: Set<http.HttpRequest> | undefined = this.activeRequests.get(requestTag);
      if (requests === undefined) {
        requests = new Set<http.HttpRequest>();
        this.activeRequests.set(requestTag, requests);
      }
      requests.add(request);
    }
    const headers: HttpHeaders = { Accept: 'application/json', 'Content-Type': 'application/json' };
    if (token !== undefined) headers.Authorization = `Bearer ${token}`;
    // Only the read side takes the caller's budget. Connecting is connecting on every endpoint, and
    // a chat request to an unreachable host should still fail in seconds rather than in two minutes.
    const options = noRedirectOptions({
      method: this.method(method),
      header: headers,
      connectTimeout: 30000,
      readTimeout: timeoutMs === undefined ? 30000 : timeoutMs,
      extraData: body === undefined ? undefined : JSON.stringify(body)
    });
    try {
      const response = await request.request(`${ACCOUNT_ORIGIN}${path}`, options);
      if (path.startsWith('/v1/auth/') && response.responseCode >= 400) {
        // 登录失败到页面上只剩一个映射后的 account_* 码，500 和网络错误都成了 account_unavailable。这里留下状态和后端自己的错误码，便于定位；只记形如 invalid_credentials 的码，不记正文、令牌或凭据。
        KeyboardLog.warn(`account ${method} ${path.split('?')[0]} -> ${response.responseCode} ${backendErrorCode(response.result)}`);
      }
      return {
        status: response.responseCode,
        body: typeof response.result === 'string' ? response.result : '',
        contentLength: responseContentLength(response.header)
      };
    } finally {
      if (requestTag !== undefined && requestTag.length > 0) {
        const requests: Set<http.HttpRequest> | undefined = this.activeRequests.get(requestTag);
        requests?.delete(request);
        if (requests !== undefined && requests.size === 0) this.activeRequests.delete(requestTag);
      }
      request.destroy();
    }
  }

  cancelRequests(requestTag: string): void {
    const requests: Set<http.HttpRequest> | undefined = this.activeRequests.get(requestTag);
    if (requests === undefined) return;
    for (const request of requests) request.destroy();
    this.activeRequests.delete(requestTag);
  }

  private method(value: string): http.RequestMethod {
    switch (value) {
      case 'POST': return http.RequestMethod.POST;
      case 'PUT': return http.RequestMethod.PUT;
      // The current SDK enum omits PATCH, but the HTTP stack accepts the wire method.
      case 'PATCH': return 'PATCH' as http.RequestMethod;
      case 'DELETE': return http.RequestMethod.DELETE;
      default: return http.RequestMethod.GET;
    }
  }
}
