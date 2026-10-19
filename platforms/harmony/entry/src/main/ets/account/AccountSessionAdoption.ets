import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { SharedStateMirror } from '../keyboard/SharedStateMirror';
import {
  ABSENT_FINGERPRINT, KEYBOARD_ENTRIES, ManifestEntry, MirrorEntry, MirrorManifest, MirrorPublisher, SETTINGS_ENTRIES
} from '../keyboard/SharedStateMirrorPolicy';
import {
  ADOPTED_SESSIONS, AdoptedSession, AdoptionGate, AdoptionResult, AdoptionTracker, adoptionFingerprint, adoptionResult,
  sessionAdoptRequest
} from './AccountSessionAdoptionPolicy';

/**
 * 本进程的会话镜像队列（迁移差异 M2）。设置应用和键盘各有一份：设置应用采用键盘发布的会话（`from:"keyboard"`），键盘采用设置应用发布的（`from:"settings"`）。
 *
 * 每个认证请求在调 Rust 之前都要先 `await AccountSessionAdoption.beforeRequest()`：它再排一次采用并等它做完，本沙箱不拿手里可能已经被另一边用过的刷新令牌去刷新。不靠生命周期事件的先后：网页回到可见时发的请求可能早于窗口的 ACTIVE 事件。规则在 `AccountSessionAdoptionPolicy` 的 `AdoptionGate`。
 */
export class AccountSessionAdoption {
  private static readonly gate: AdoptionGate = new AdoptionGate();
  private static readonly tracker: AdoptionTracker = new AdoptionTracker();

  /** 认证请求之前排一次采用并等它做完（连同排在它前面的）。 */
  static beforeRequest(): Promise<void> {
    return AccountSessionAdoption.gate.beforeRequest();
  }

  /**
   * 进程启动时登记本进程的采用并排一次：共享目录要异步才拿得到，所以在拿到之前就先占住队列。之后每个认证请求的 `beforeRequest` 都再跑一次这里登记的采用。拿不到共享目录（签名不带 data-group-id、模拟器）时什么也不做；`enabled` 为假（键盘在基础访问模式下不交换会话）时也不做，每次都读当前值。
   */
  static adoptOnStart(mirror: Promise<SharedStateMirror | null>, stateDirectory: string,
    publisher: MirrorPublisher, enabled: () => boolean): void {
    AccountSessionAdoption.gate.register(async (): Promise<void> => {
      const resolved: SharedStateMirror | null = await mirror;
      if (resolved !== null && enabled()) {
        await AccountSessionAdoption.adoptNow(resolved, stateDirectory, publisher);
      }
    });
  }

  /** 回到前台时排一次采用，与发布、导入其他条目的镜像同一时刻，让本地显示的账号状态尽早跟上。认证请求的顺序不靠它，靠 `beforeRequest`。 */
  static adopt(mirror: SharedStateMirror, stateDirectory: string, publisher: MirrorPublisher): Promise<void> {
    return AccountSessionAdoption.gate.run((): Promise<void> =>
      AccountSessionAdoption.adoptNow(mirror, stateDirectory, publisher));
  }

  private static async adoptNow(mirror: SharedStateMirror, stateDirectory: string,
    publisher: MirrorPublisher): Promise<void> {
    // 发布方从来没有发布过（没有清单）时不采用：缺的是整份镜像，不是「对方退出了登录」。
    const manifest: MirrorManifest | null = mirror.manifest(publisher);
    if (manifest === null) return;
    const entries: MirrorEntry[] = publisher === 'settings' ? SETTINGS_ENTRIES : KEYBOARD_ENTRIES;
    for (const session of ADOPTED_SESSIONS) {
      await AccountSessionAdoption.adoptOne(mirror, stateDirectory, publisher, manifest, entries, session);
    }
  }

  private static async adoptOne(mirror: SharedStateMirror, stateDirectory: string, publisher: MirrorPublisher,
    manifest: MirrorManifest, entries: MirrorEntry[], session: AdoptedSession): Promise<void> {
    const published: ManifestEntry | undefined =
      manifest.entries.find((entry: ManifestEntry): boolean => entry.name === session.entry);
    const entry: MirrorEntry | undefined =
      entries.find((candidate: MirrorEntry): boolean => candidate.name === session.entry);
    if (published === undefined || entry === undefined) return;
    // 匿名会话连同身份一起交给 Rust：身份文件的路径总是给，文件不在就是发布方没有身份。
    const identityEntry: MirrorEntry | undefined = session.identityEntry === null ? undefined :
      entries.find((candidate: MirrorEntry): boolean => candidate.name === session.identityEntry);
    const identityPublished: ManifestEntry | undefined = session.identityEntry === null ? undefined :
      manifest.entries.find((candidate: ManifestEntry): boolean => candidate.name === session.identityEntry);
    const identityPath: string | null =
      identityEntry === undefined ? null : mirror.publishedFile(publisher, identityEntry);
    const fingerprint: string = adoptionFingerprint(published.fingerprint, identityEntry === undefined ? null :
      (identityPublished === undefined ? ABSENT_FINGERPRINT : identityPublished.fingerprint));
    const tracker: AdoptionTracker = AccountSessionAdoption.tracker;
    if (!tracker.due(publisher, session.item, fingerprint)) return;
    const path: string | null =
      published.fingerprint === ABSENT_FINGERPRINT ? null : mirror.publishedFile(publisher, entry);
    let reply: string;
    try {
      reply = await client.account(sessionAdoptRequest(stateDirectory, path, session.item, publisher, identityPath));
    } catch (error) {
      KeyboardLog.error(`session adopt from ${publisher} did not answer: ${describeError(error)}`);
      return;
    }
    const result: AdoptionResult = adoptionResult(reply);
    if (result.outcome.length > 0) {
      tracker.answered(publisher, session.item, fingerprint, result);
      if (result.outcome !== 'unchanged') {
        KeyboardLog.info(`session ${result.outcome} from ${publisher} (${session.item})`);
      }
      return;
    }
    const message: string | null = tracker.failed(publisher, session.item, fingerprint, result.error);
    if (message !== null) KeyboardLog.error(message);
  }
}
