import http from '@ohos.net.http';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import { AccountSessionStore, AccountTransport, AccountTransportResponse } from './AccountCloudBridge';
import { installNoRedirectGuard, noRedirectOptions } from '../network/HarmonyHttpSecurity';

/**
 * The account session file and the HTTPS transport, shared by the settings application and the keyboard extension.
 *
 * Both processes belong to the one `entry` module, so `<files>/state` is the same sandbox directory in each, and the keyboard reads the session the settings page signed in with rather than holding a copy of its own. Nothing here is exported to a WebView.
 */
export const ACCOUNT_ORIGIN: string = 'https://api.msime.app';
const MAX_SESSION_BYTES: number = 64 * 1024;

/**
 * `<state>/account-session.json`, written whole and then renamed into place.
 *
 * Two processes read this file. `AccountCloudBridge` refuses and clears a document it cannot parse, so a reader that caught a truncate-then-write halfway would sign the user out; writing a uniquely named sibling and renaming it means a reader only ever sees the old document or the new one.
 */
export class FileSessionStore implements AccountSessionStore {
  private readonly directory: string;
  private readonly file: string;
  private readonly lockFile: string;

  constructor(directory: string) {
    this.directory = directory;
    this.file = `${directory}/account-session.json`;
    this.lockFile = `${directory}/account-session.lock`;
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
    try {
      if (!this.safeDirectory()) return null;
      const stat: fs.Stat = fs.lstatSync(this.file);
      if (stat.isSymbolicLink() || !stat.isFile() || stat.size > MAX_SESSION_BYTES) return null;
      return fs.readTextSync(this.file);
    } catch {
      return null;
    }
  }

  save(value: string): void {
    if (value.length > MAX_SESSION_BYTES || !this.prepareDirectory() || this.isSymbolicLink(this.file)) {
      throw new Error('account_storage');
    }
    const staging: string = `${this.file}.${util.generateRandomUUID(false)}.tmp`;
    let renamed: boolean = false;
    try {
      const handle = fs.openSync(staging, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(handle.fd, value);
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
  const parsed: number = Number(raw);
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : -1;
}

export function responseContentType(headers: Object): string | undefined {
  const raw: string | undefined = responseHeader(headers, 'content-type');
  return raw === undefined ? undefined : raw.split(';')[0].trim().toLowerCase();
}

/** JSON requests to the account service through the system HTTPS stack; the settings application adds file transfers on top. */
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
