/** 两个沙箱直接读取同一份已校验资源，偏好、用户词库和缓存仍在各自私有目录。 */
import fs from '@ohos.file.fs';
import client from 'libmsimeclient.so';
import { SharedStateMirror } from './SharedStateMirror';
import { KeyboardLog, describeError } from './KeyboardLog';
import { DictionaryResourceAccess, SHARED_DICTIONARY_PACKS, SharedDictionaryResourcePolicy } from './SharedDictionaryResourcePolicy';

interface PackStatus { id: string; state: string; }
interface PackListReply { ok: boolean; value?: PackStatus[]; }
interface PackReply { ok: boolean; }

class NativeDictionaryResourceAccess implements DictionaryResourceAccess {
  installed(root: string, pack: string): boolean {
    const reply: PackListReply = JSON.parse(client.resourcePacks(JSON.stringify({ state_root: root }))) as PackListReply;
    return reply.ok === true && reply.value !== undefined
      && reply.value.some((item: PackStatus): boolean => item.id === pack && item.state === 'installed');
  }

  present(root: string, pack: string): boolean {
    try { return fs.lstatSync(`${root}/resource-packs/${pack}`).isDirectory(); }
    catch (_) { return false; }
  }

  async adopt(root: string, pack: string, source: string): Promise<boolean> {
    const result: string = await client.resourcePackAdopt(JSON.stringify({ state_root: root, pack: pack, source: source }));
    return (JSON.parse(result) as PackReply).ok === true;
  }

  remove(root: string, pack: string): void {
    client.resourcePackRemove(JSON.stringify({ state_root: root, pack: pack }));
  }
}

export class SharedDictionaryResources {
  private pending: Promise<void> | null = null;

  private readonly mirror: SharedStateMirror;
  private readonly privateRoot: string;

  constructor(mirror: SharedStateMirror, privateRoot: string) {
    this.mirror = mirror;
    this.privateRoot = privateRoot;
  }

  root(): string { return this.mirror.dictionaryResourceRoot(); }

  rootForPack(pack: string): string {
    return SHARED_DICTIONARY_PACKS.includes(pack) ? this.root() : this.privateRoot;
  }

  list(): string {
    const local: PackListReply = JSON.parse(client.resourcePacks(JSON.stringify({ state_root: this.privateRoot }))) as PackListReply;
    const shared: PackListReply = JSON.parse(client.resourcePacks(JSON.stringify({ state_root: this.root() }))) as PackListReply;
    if (local.ok !== true || local.value === undefined) return JSON.stringify(local);
    if (shared.ok !== true || shared.value === undefined) return JSON.stringify(shared);
    for (const item of shared.value) {
      if (!SHARED_DICTIONARY_PACKS.includes(item.id)) continue;
      const index: number = local.value.findIndex((entry: PackStatus): boolean => entry.id === item.id);
      if (index >= 0) local.value[index] = item;
    }
    return JSON.stringify(local);
  }

  remove(pack: string): string {
    const root: string = this.rootForPack(pack);
    if (SHARED_DICTIONARY_PACKS.includes(pack)) {
      // 先清理可被再次收编的来源；失败时保留共享资源，避免假成功或升级后复活。
      const legacyRoots: string[] = [this.privateRoot, this.mirror.legacyDictionaryResourceRoot()];
      for (const legacy of legacyRoots) {
        if (legacy === root) continue;
        const result: string = client.resourcePackRemove(JSON.stringify({ state_root: legacy, pack: pack }));
        if ((JSON.parse(result) as PackReply).ok !== true) return result;
      }
    }
    return client.resourcePackRemove(JSON.stringify({ state_root: root, pack: pack }));
  }


  prepare(): Promise<void> {
    if (this.pending !== null) return this.pending;
    const legacyRoots: string[] = [this.privateRoot, this.mirror.legacyDictionaryResourceRoot()];
    this.pending = SharedDictionaryResourcePolicy.migrate(this.root(), legacyRoots,
      new NativeDictionaryResourceAccess()).catch((error: Object): void => {
      KeyboardLog.error(`shared dictionary resource migration failed: ${describeError(error)}`);
    }).finally((): void => { this.pending = null; });
    return this.pending;
  }

  static removePrivateCopies(mirror: SharedStateMirror, privateRoot: string): void {
    SharedDictionaryResourcePolicy.removePrivateCopies(mirror.dictionaryResourceRoot(), privateRoot,
      new NativeDictionaryResourceAccess());
  }
}
