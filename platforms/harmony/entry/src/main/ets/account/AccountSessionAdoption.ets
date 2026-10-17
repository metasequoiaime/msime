import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { SharedStateMirror } from '../keyboard/SharedStateMirror';
import {
  ABSENT_FINGERPRINT, KEYBOARD_ENTRIES, ManifestEntry, MirrorEntry, MirrorManifest, MirrorPublisher, SETTINGS_ENTRIES
} from '../keyboard/SharedStateMirrorPolicy';
import {
  ADOPTED_SESSIONS, AdoptedSession, AdoptionResult, AdoptionTracker, SerialGate, adoptionResult, sessionAdoptRequest
} from './AccountSessionAdoptionPolicy';

/**
 * 本进程的会话镜像队列（迁移差异 M2）。设置应用和键盘各有一份：设置应用采用键盘发布的会话（`from:"keyboard"`），键盘采用设置应用发布的（`from:"settings"`）。
 *
 * 每个认证请求在调 Rust 之前都要先 `await AccountSessionAdoption.settled()`：进程启动时和每次回到前台时排进来的采用做完以前，本沙箱不拿手里可能已经被另一边用过的刷新令牌去刷新。规则在 `AccountSessionAdoptionPolicy`。
 */
export class AccountSessionAdoption {
  private static readonly gate: SerialGate = new SerialGate();
  private static readonly tracker: AdoptionTracker = new AdoptionTracker();

  /** 此刻已经排进来的采用都做完。认证请求先等它。 */
  static settled(): Promise<void> {
    return AccountSessionAdoption.gate.settled();
  }

  /**
   * 进程启动时排一次采用：共享目录要异步才拿得到，所以在拿到之前就先占住队列，之后的认证请求都排在它后面。拿不到共享目录（签名不带 data-group-id、模拟器）时什么也不做；`enabled` 为假（键盘在基础访问模式下不交换会话）时也不做。
   */
  static adoptOnStart(mirror: Promise<SharedStateMirror | null>, stateDirectory: string,
    publisher: MirrorPublisher, enabled: () => boolean): void {
    AccountSessionAdoption.gate.run(async (): Promise<void> => {
      const resolved: SharedStateMirror | null = await mirror;
      if (resolved !== null && enabled()) {
        await AccountSessionAdoption.adoptNow(resolved, stateDirectory, publisher);
      }
    });
  }

  /** 回到前台时排一次采用，与发布、导入其他条目的镜像同一时刻。 */
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
    const tracker: AdoptionTracker = AccountSessionAdoption.tracker;
    if (!tracker.due(publisher, session.item, published.fingerprint)) return;
    const path: string | null =
      published.fingerprint === ABSENT_FINGERPRINT ? null : mirror.publishedFile(publisher, entry);
    let reply: string;
    try {
      reply = await client.account(sessionAdoptRequest(stateDirectory, path, session.item, publisher));
    } catch (error) {
      KeyboardLog.error(`session adopt from ${publisher} did not answer: ${describeError(error)}`);
      return;
    }
    const result: AdoptionResult = adoptionResult(reply);
    if (result.outcome.length > 0) {
      tracker.succeeded(publisher, session.item, published.fingerprint);
      if (result.outcome !== 'unchanged') {
        KeyboardLog.info(`session ${result.outcome} from ${publisher} (${session.item})`);
      }
      return;
    }
    const message: string | null = tracker.failed(publisher, session.item, published.fingerprint, result.error);
    if (message !== null) KeyboardLog.error(message);
  }
}
