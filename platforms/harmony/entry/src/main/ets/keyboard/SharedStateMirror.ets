/**
 * 共享沙箱里的状态镜像：把这一侧的条目发布到 `<共享目录>/msime/<发布方>/`，或把对方发布的条目导入自己的 `files`。规则见 `SharedStateMirrorPolicy`。
 *
 * 只用同步文件操作，调用方都在主线程上的生命周期时刻（键盘获得焦点、设置窗口回到前台或改完设置之后）调用；没有变化时只读一份很小的清单。文件先写到旁边再改名换入，目录先整棵复制到旁边再整体替换，对方读到的要么是旧的、要么是新的。
 */
import fs from '@ohos.file.fs';
import common from '@ohos.app.ability.common';
import util from '@ohos.util';
import { KeyboardLog, describeError } from './KeyboardLog';
import { StagedResources } from './StagedResources';
import {
  ABSENT_FINGERPRINT,
  FingerprintedEntry,
  ImportDecision,
  ImportedEntry,
  ImportRecord,
  ListedFile,
  MANIFEST_FILE,
  ManifestEntry,
  MirrorEntry,
  MirrorManifest,
  MirrorPublisher,
  PublishPlan,
  SharedStateMirrorPolicy,
} from './SharedStateMirrorPolicy';

/** AppGallery Connect 为「输入法应用内数据共享」签发的 data-group-id，签名 Profile 的 `data-group-ids` 和键盘扩展在 `module.json5` 里声明的 `dataGroupIds` 必须与它一致。 */
export const DATA_GROUP_ID: string = 'group.1516753898738609107';

/** 导入时由调用方处理的条目：偏好要经 `savePreferences` 写入，不能直接覆盖文件。返回是否已经处理好；返回假时这一条下次再试。 */
export interface MirrorImportHandlers {
  preferences: (documentText: string) => boolean;
  /** 导入前看一眼某个普通文件的新内容（为空表示对方删除了它），决定照常复制、自己处理还是下次再试。 */
  beforeFile: (name: string, text: string | null) => ImportDecision;
}

export class SharedStateMirror {
  private readonly groupRoot: string;
  private readonly filesDir: string;

  constructor(groupDir: string, filesDir: string) {
    this.groupRoot = `${groupDir}/msime`;
    this.filesDir = filesDir;
  }

  /**
   * 这个进程的共享目录，没有时为 null：签名 Profile 不带这个 data-group-id、`module.json5` 没有声明，或者在模拟器上（不做这项授权校验，会报错）。拿不到时两边仍各用各的 `files/state`，与没有共享沙箱时相同。
   */
  static async resolve(context: common.Context): Promise<string | null> {
    try {
      const directory: string = await context.getGroupDir(DATA_GROUP_ID);
      if (typeof directory !== 'string' || directory.length === 0) {
        KeyboardLog.warn('shared sandbox unavailable: no group directory for the data group id');
        return null;
      }
      return directory;
    } catch (error) {
      KeyboardLog.warn(`shared sandbox unavailable: ${describeError(error)}`);
      return null;
    }
  }

  /** 这一侧当前的指纹，按 `entries` 的顺序。 */
  fingerprints(entries: MirrorEntry[]): FingerprintedEntry[] {
    return entries.map((entry: MirrorEntry): FingerprintedEntry => {
      return { name: entry.name, fingerprint: SharedStateMirror.fingerprintOf(entry, this.localPath(entry)) };
    });
  }

  /** 把变化了的条目复制到共享目录，最后写清单。返回复制了哪些条目；共享目录写不进去时返回空并记日志。 */
  publish(publisher: MirrorPublisher, entries: MirrorEntry[], applied: ImportedEntry[], nowMs: number): string[] {
    try {
      const previous: MirrorManifest | null = this.manifest(publisher);
      const plan: PublishPlan = SharedStateMirrorPolicy.plan(previous, publisher,
        this.fingerprints(entries), nowMs, applied);
      const sameApplied: boolean = previous !== null
        && JSON.stringify(previous.applied) === JSON.stringify(applied);
      if (plan.changed.length === 0 && sameApplied) {
        return [];
      }
      for (const name of plan.changed) {
        const entry: MirrorEntry | undefined = entries.find((item: MirrorEntry): boolean => item.name === name);
        if (entry === undefined) continue;
        SharedStateMirror.copy(entry.kind === 'directory', this.localPath(entry),
          this.publishedPath(publisher, entry));
      }
      SharedStateMirror.writeText(`${this.publisherRoot(publisher)}/${MANIFEST_FILE}`,
        JSON.stringify(plan.manifest));
      return plan.changed;
    } catch (error) {
      KeyboardLog.error(`shared state not published: ${describeError(error)}`);
      return [];
    }
  }

  /**
   * 导入对方发布的、与上次导入不同的条目。`own` 是这一侧自己的发布清单（不发布的一侧传 null），用来判断哪边改得更晚。返回导入了哪些条目。
   */
  importFrom(other: MirrorPublisher, entries: MirrorEntry[], fullMode: boolean,
    handlers: MirrorImportHandlers): string[] {
    const imported: string[] = [];
    try {
      const own: MirrorManifest | null = this.manifest(other === 'settings' ? 'keyboard' : 'settings');
      let record: ImportRecord | null = this.importRecord(other);
      const pending: ManifestEntry[] = SharedStateMirrorPolicy.pending(this.manifest(other), record, own,
        entries, fullMode);
      for (const published of pending) {
        const entry: MirrorEntry | undefined =
          entries.find((item: MirrorEntry): boolean => item.name === published.name);
        if (entry === undefined) continue;
        if (!this.importEntry(other, entry, published.fingerprint === ABSENT_FINGERPRINT, handlers)) continue;
        record = SharedStateMirrorPolicy.recordImport(record, entry.name, published.fingerprint);
        imported.push(entry.name);
      }
      if (imported.length > 0 && record !== null) {
        SharedStateMirror.writeText(this.recordPath(other), JSON.stringify(record));
      }
    } catch (error) {
      KeyboardLog.error(`shared state not imported: ${describeError(error)}`);
    }
    return imported;
  }

  /** 对方的发布清单，读不到时为 null。 */
  manifest(publisher: MirrorPublisher): MirrorManifest | null {
    const text: string | null = SharedStateMirror.readText(`${this.publisherRoot(publisher)}/${MANIFEST_FILE}`);
    return text === null ? null : SharedStateMirrorPolicy.parseManifest(text);
  }

  /** 这一侧从 `publisher` 导入到了哪一版。 */
  importRecord(publisher: MirrorPublisher): ImportRecord | null {
    const text: string | null = SharedStateMirror.readText(this.recordPath(publisher));
    return text === null ? null : SharedStateMirrorPolicy.parseRecord(text);
  }

  /** 对方发布的某个普通文件的内容，没有时为 null。 */
  publishedText(publisher: MirrorPublisher, entry: MirrorEntry): string | null {
    return SharedStateMirror.readText(this.publishedPath(publisher, entry));
  }

  localPath(entry: MirrorEntry): string {
    return entry.base === 'state' ? `${this.filesDir}/state/${entry.path}` : `${this.filesDir}/${entry.path}`;
  }

  private importEntry(other: MirrorPublisher, entry: MirrorEntry, absent: boolean,
    handlers: MirrorImportHandlers): boolean {
    const source: string = this.publishedPath(other, entry);
    if (entry.kind === 'preferences') {
      if (absent) return true;
      const text: string | null = SharedStateMirror.readText(source);
      return text !== null && handlers.preferences(text);
    }
    if (entry.kind === 'file') {
      const decision: ImportDecision = handlers.beforeFile(entry.name, absent ? null : SharedStateMirror.readText(source));
      if (decision === 'retry') return false;
      if (decision === 'handled') return true;
    }
    const destination: string = this.localPath(entry);
    if (absent) {
      SharedStateMirror.remove(destination);
      return true;
    }
    SharedStateMirror.copy(entry.kind === 'directory', source, destination);
    return true;
  }

  private publisherRoot(publisher: MirrorPublisher): string {
    return `${this.groupRoot}/${publisher}`;
  }

  private publishedPath(publisher: MirrorPublisher, entry: MirrorEntry): string {
    return `${this.publisherRoot(publisher)}/${entry.base}/${entry.path}`;
  }

  private recordPath(publisher: MirrorPublisher): string {
    return `${this.filesDir}/state/shared-import-${publisher}.json`;
  }

  private static fingerprintOf(entry: MirrorEntry, path: string): string {
    try {
      let stat: fs.Stat;
      try {
        stat = fs.lstatSync(path);
      } catch {
        return ABSENT_FINGERPRINT;
      }
      if (stat.isSymbolicLink()) return ABSENT_FINGERPRINT;
      if (entry.kind === 'directory') {
        if (!stat.isDirectory()) return ABSENT_FINGERPRINT;
        const files: ListedFile[] = [];
        SharedStateMirror.listFiles(path, '', files);
        return SharedStateMirrorPolicy.directoryFingerprint(files);
      }
      if (!stat.isFile()) return ABSENT_FINGERPRINT;
      const text: string | null = SharedStateMirror.readText(path);
      if (text === null) return ABSENT_FINGERPRINT;
      return entry.kind === 'preferences' ? SharedStateMirrorPolicy.preferencesFingerprint(text)
        : SharedStateMirrorPolicy.fingerprintText(text);
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateMirror.fingerprintOf failed');
      return ABSENT_FINGERPRINT;
    }
  }

  /** `root` 下的每个普通文件。锁文件和暂存文件不算内容，跳过，否则对方一读一加锁指纹就变。 */
  private static listFiles(root: string, relative: string, into: ListedFile[]): void {
    try {
      const directory: string = relative.length === 0 ? root : `${root}/${relative}`;
      for (const name of fs.listFileSync(directory)) {
        if (SharedStateMirror.transient(name)) continue;
        const path: string = relative.length === 0 ? name : `${relative}/${name}`;
        const stat: fs.Stat = fs.lstatSync(`${root}/${path}`);
        if (stat.isSymbolicLink()) continue;
        if (stat.isDirectory()) {
          SharedStateMirror.listFiles(root, path, into);
        } else if (stat.isFile()) {
          into.push({ path: path, size: stat.size, mtime: stat.mtime });
        }
      }
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateMirror.listFiles failed');
      throw error as Error;
    }
  }

  private static transient(name: string): boolean {
    return name.endsWith('.lock') || name.endsWith('.tmp') || name.includes('.staging')
      || name.includes('.previous') || name.startsWith('.import-');
  }

  private static copy(directory: boolean, source: string, destination: string): void {
    try {
      let stat: fs.Stat | null = null;
      try {
        stat = fs.lstatSync(source);
      } catch {
        stat = null;
      }
      if (stat === null || stat.isSymbolicLink() || (directory ? !stat.isDirectory() : !stat.isFile())) {
        SharedStateMirror.remove(destination);
        return;
      }
      SharedStateMirror.ensureParent(destination);
      const staging: string = `${destination}.${util.generateRandomUUID(false)}.staging`;
      if (!directory) {
        try {
          fs.copyFileSync(source, staging);
          fs.renameSync(staging, destination);
        } finally {
          try {
            if (fs.accessSync(staging)) fs.unlinkSync(staging);
          } catch {
            // 只会留下这个唯一命名的暂存文件。
          }
        }
        return;
      }
      try {
        SharedStateMirror.copyTree(source, staging);
        StagedResources.replaceDirectory(staging, destination);
      } catch (error) {
        try {
          if (fs.accessSync(staging)) StagedResources.removeDirectory(staging);
        } catch {
          // 保留下面抛出的那个错误。
        }
        throw error as Error;
      }
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateMirror.copy failed');
      throw error as Error;
    }
  }

  private static copyTree(source: string, destination: string): void {
    try {
      fs.mkdirSync(destination, true);
      for (const name of fs.listFileSync(source)) {
        if (SharedStateMirror.transient(name)) continue;
        const from: string = `${source}/${name}`;
        const to: string = `${destination}/${name}`;
        const stat: fs.Stat = fs.lstatSync(from);
        if (stat.isSymbolicLink()) continue;
        if (stat.isDirectory()) {
          SharedStateMirror.copyTree(from, to);
        } else if (stat.isFile()) {
          fs.copyFileSync(from, to);
        }
      }
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateMirror.copyTree failed');
      throw error as Error;
    }
  }

  private static remove(path: string): void {
    try {
      let stat: fs.Stat;
      try {
        stat = fs.lstatSync(path);
      } catch {
        return;
      }
      if (stat.isDirectory() && !stat.isSymbolicLink()) {
        StagedResources.removeDirectory(path);
      } else {
        fs.unlinkSync(path);
      }
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateMirror.remove failed');
      throw error as Error;
    }
  }

  private static ensureParent(path: string): void {
    try {
      const parent: string = path.substring(0, path.lastIndexOf('/'));
      if (parent.length > 0 && !fs.accessSync(parent)) {
        fs.mkdirSync(parent, true);
      }
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateMirror.ensureParent failed');
      throw error as Error;
    }
  }

  private static readText(path: string): string | null {
    try {
      const stat: fs.Stat = fs.lstatSync(path);
      if (stat.isSymbolicLink() || !stat.isFile()) return null;
      return fs.readTextSync(path);
    } catch {
      return null;
    }
  }

  private static writeText(path: string, text: string): void {
    try {
      SharedStateMirror.ensureParent(path);
      const staging: string = `${path}.${util.generateRandomUUID(false)}.tmp`;
      try {
        const handle: fs.File = fs.openSync(staging,
          fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
        try {
          fs.writeSync(handle.fd, text);
        } finally {
          fs.closeSync(handle);
        }
        fs.renameSync(staging, path);
      } finally {
        try {
          if (fs.accessSync(staging)) fs.unlinkSync(staging);
        } catch {
          // 只会留下这个唯一命名的暂存文件。
        }
      }
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateMirror.writeText failed');
      throw error as Error;
    }
  }
}
