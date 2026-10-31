import { NativeCleanup } from './NativeCleanup';
/**
 * 设置应用和键盘各自在什么时候发布、导入共享沙箱里的状态，以及几条要经原生存储写入、不能直接覆盖文件的条目怎么处理。文件层面的复制在 `SharedStateMirror`，规则在 `SharedStateMirrorPolicy`。
 */
import client from 'libmsimeclient.so';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import { KeyboardLog, describeError } from './KeyboardLog';
import { SharedStateMirror, MirrorImportHandlers } from './SharedStateMirror';
import {
  ImportDecision,
  KEYBOARD_ENTRIES,
  KEYBOARD_PERSONAL_WORDBOOK_DRAINED_ENTRY,
  KEYBOARD_PERSONAL_WORDBOOK_ENTRY,
  ImportRecord,
  MirrorEntry,
  MirrorManifest,
  SETTINGS_ENTRIES,
  STATISTICS_SWITCH_FILE,
  SharedStateMirrorPolicy,
  StatisticsSwitch,
} from './SharedStateMirrorPolicy';
import { TypingStatisticsPolicy } from './TypingStatisticsPolicy';
import {
  KEYBOARD_PERSONAL_WORDBOOK_DRAINED_PATH,
  KEYBOARD_PERSONAL_WORDBOOK_PATH,
  PersonalWordbookPolicy,
} from './PersonalWordbookPolicy';

/** `typingStatistics` 的动作，与设置页发给它的同形。 */
interface StatisticsAction {
  operation: string;
  enabled?: boolean;
  retention?: string;
  day?: string;
}

interface StatisticsRequest {
  directory: string;
  action: StatisticsAction;
}

export class SharedStateSync {
  /**
   * 设置应用一侧：先发布自己的改动，再导入键盘在完整体验模式下发布的改动。设置窗口回到前台时调用，接在它之后的 `refreshPreferencesIfChanged` 会把导入的偏好交给页面。返回导入了哪些条目。
   */
  static settingsSync(mirror: SharedStateMirror, stateDirectory: string): string[] {
    SharedStateSync.settingsPublish(mirror, stateDirectory);
    const switchText: string | null = SharedStateSync.readText(`${stateDirectory}/${STATISTICS_SWITCH_FILE}`);
    const switchFingerprint: string = switchText === null ? '' : SharedStateMirrorPolicy.fingerprintText(switchText);
    const keyboard: MirrorManifest | null = mirror.manifest('keyboard');
    const handlers: MirrorImportHandlers = {
      preferences: (text: string): boolean => SharedStateSync.importPreferences(stateDirectory, text),
      beforeFile: (name: string, text: string | null): ImportDecision => {
        // 键盘的统计文件带着它当时用的开关和保留期，只有它已经照设置应用当前的开关记录时才能采用；采用时与设置应用自己记的皮肤和徽章合并，不整体覆盖。
        if (name === 'typing-statistics') {
          if (text === null || !SharedStateMirrorPolicy.statisticsInStep(keyboard, switchFingerprint)) {
            return 'retry';
          }
          return SharedStateSync.adoptKeyboardStatistics(stateDirectory, text) ? 'handled' : 'retry';
        }
        // 键盘那本生词本和它的清走记录：两条都交给同一个合并，它自己读共享目录里的两份文件、管「上次并过的那一版」，不照常复制。
        if (name === KEYBOARD_PERSONAL_WORDBOOK_ENTRY || name === KEYBOARD_PERSONAL_WORDBOOK_DRAINED_ENTRY) {
          return SharedStateSync.mergeKeyboardPersonalWordbook(mirror, stateDirectory) ? 'handled' : 'retry';
        }
        return SharedStateSync.accountFileDecision(name);
      },
    };
    const imported: string[] = mirror.importFrom('keyboard', KEYBOARD_ENTRIES, true, handlers);
    if (imported.length > 0) {
      KeyboardLog.info(`shared state imported from the keyboard: ${imported.join(', ')}`);
    }
    // 并过键盘那本以后马上把并到了哪一版发布出去，键盘下次获得焦点就能清走交过的词。
    if (imported.includes(KEYBOARD_PERSONAL_WORDBOOK_ENTRY) || imported.includes(KEYBOARD_PERSONAL_WORDBOOK_DRAINED_ENTRY)) {
      SharedStateSync.settingsPublish(mirror, stateDirectory);
    }
    return imported;
  }

  /**
   * 两份会话和匿名账号的身份不经文件复制导入：Rust 的 `session_adopt` 在本沙箱的 `account-refresh.lock` 里按 user.id 和 expires_at 决定要不要采用（`AccountSessionAdoption`，迁移差异 M5），整份覆盖会把轮换后的令牌换回用过的那枚，或者让键盘的旧副本撤销设置应用里的换号。身份随匿名会话一起采用，会话和身份始终属于同一个匿名账号（迁移差异 M40）。发布方删掉了会话或身份时也不在这里删本地文件，是否跟着清掉同样由 Rust 决定。规则在 `SharedStateMirrorPolicy.importDecision`。
   */
  static accountFileDecision(name: string): ImportDecision {
    return SharedStateMirrorPolicy.importDecision(name);
  }

  /** 设置应用一侧：发布设置应用管的那些条目。改完设置之后和窗口退到后台时调用。 */
  static settingsPublish(mirror: SharedStateMirror, stateDirectory: string): void {
    try {
      if (!fs.accessSync(`${stateDirectory}/${STATISTICS_SWITCH_FILE}`)) {
        SharedStateSync.writeStatisticsSwitch(stateDirectory, false);
      }
      const published: string[] = mirror.publish('settings', SETTINGS_ENTRIES,
        SharedStateMirrorPolicy.personalWordbookApplied(mirror.importRecord('keyboard')), Date.now());
      if (published.length > 0) {
        KeyboardLog.info(`shared state published for the keyboard: ${published.join(', ')}`);
      }
    } catch (error) {
      KeyboardLog.error('keyboard.SharedStateSync.settingsPublish failed');
      throw error as Error;
    }
  }

  /**
   * 键盘一侧：完整体验模式下先发布键盘自己的改动（附上它已导入到设置应用的哪一版），再导入设置应用发布的改动。键盘获得焦点、在已有偏好修订号比较之前调用，导入的偏好由随后的比较发现并重建会话。
   */
  static keyboardSync(mirror: SharedStateMirror, stateDirectory: string, fullMode: boolean): string[] {
    if (fullMode) {
      SharedStateSync.keyboardDrainPersonalWordbook(mirror, stateDirectory);
      SharedStateSync.keyboardPublish(mirror);
    }
    const handlers: MirrorImportHandlers = {
      preferences: (text: string): boolean => SharedStateSync.importPreferences(stateDirectory, text),
      beforeFile: (name: string, text: string | null): ImportDecision => {
        if (name === 'statistics-switch') {
          return SharedStateSync.applyStatisticsSwitch(stateDirectory, text) ? 'copy' : 'retry';
        }
        // 设置应用还没有云剪贴板的设备身份时，键盘这份留着不删；有了就照常整份复制过来。
        if (text === null && SharedStateMirrorPolicy.keepsLocalWhenAbsent(name)) {
          return 'handled';
        }
        return SharedStateSync.accountFileDecision(name);
      },
    };
    const imported: string[] = mirror.importFrom('settings', SETTINGS_ENTRIES, fullMode, handlers);
    if (imported.length > 0) {
      KeyboardLog.info(`shared state imported from settings: ${imported.join(', ')}`);
    }
    return imported;
  }

  /** 键盘一侧：完整体验模式下发布。基础访问模式下键盘对共享目录只读，调用方不调用它。 */
  static keyboardPublish(mirror: SharedStateMirror): void {
    const record: ImportRecord | null = mirror.importRecord('settings');
    mirror.publish('keyboard', KEYBOARD_ENTRIES, record === null ? [] : record.entries, Date.now());
  }

  /**
   * 键盘一侧：设置应用确认并过键盘发布着的那本生词本时（`SharedStateMirrorPolicy.personalWordbookDrainable`），把那一版里的词从键盘自己那本里清走，并把清走记录加一。之后要发布，调用方接着调 `keyboardPublish`。只在完整体验模式下调用；返回是否清走了词。
   *
   * 清走前核对共享目录里那份的内容确实是设置应用确认的那一版：发布写到一半失败时，那份可能比清单新，新的那部分设置应用还没并过。合并和清走都在原生层取同一个词书目录的锁，与工作线程上的「加入生词本」不会互相覆盖。
   */
  static keyboardDrainPersonalWordbook(mirror: SharedStateMirror, stateDirectory: string): boolean {
    try {
      const fingerprint: string | null = SharedStateMirrorPolicy.personalWordbookDrainable(
        mirror.manifest('keyboard'), mirror.manifest('settings'));
      if (fingerprint === null) return false;
      const publishedPath: string = mirror.publishedFile('keyboard',
        SharedStateSync.keyboardEntry(KEYBOARD_PERSONAL_WORDBOOK_ENTRY));
      const publishedText: string | null = SharedStateSync.readText(publishedPath);
      if (publishedText === null || SharedStateMirrorPolicy.fingerprintText(publishedText) !== fingerprint) {
        return false;
      }
      const removed: number = PersonalWordbookPolicy.drained(client.personalWordbookDrain(
        PersonalWordbookPolicy.drainRequest(stateDirectory, publishedPath)));
      if (removed <= 0) return false;
      const markerPath: string = `${stateDirectory}/${KEYBOARD_PERSONAL_WORDBOOK_DRAINED_PATH}`;
      SharedStateSync.writeText(markerPath,
        PersonalWordbookPolicy.nextDrainedMarker(SharedStateSync.readText(markerPath), fingerprint));
      return true;
    } catch (error) {
      KeyboardLog.warn(`keyboard personal wordbook not drained yet: ${describeError(error)}`);
      return false;
    }
  }

  private static keyboardEntry(name: string): MirrorEntry {
    const entry: MirrorEntry | undefined = KEYBOARD_ENTRIES.find((item: MirrorEntry): boolean => item.name === name);
    if (entry === undefined) {
      throw new Error(`unknown keyboard mirror entry ${name}`);
    }
    return entry;
  }

  /** 用对方发布的偏好替换这一侧的偏好：经 `savePreferences`，带上本地的修订号，不直接覆盖文件。内容已经相同时不写。 */
  static importPreferences(stateDirectory: string, documentText: string): boolean {
    try {
      const incoming: Record<string, Object> = JSON.parse(documentText) as Record<string, Object>;
      const preferences: Object | undefined = incoming['preferences'];
      if (preferences === undefined || preferences === null || typeof preferences !== 'object') {
        return false;
      }
      const reply: Record<string, Object> =
        JSON.parse(client.loadPreferences(stateDirectory)) as Record<string, Object>;
      if (reply['ok'] !== true) return false;
      const local: Record<string, Object> = reply['value'] as Record<string, Object>;
      if (SharedStateMirrorPolicy.preferencesFingerprint(JSON.stringify(local))
        === SharedStateMirrorPolicy.preferencesFingerprint(documentText)) {
        return true;
      }
      const snapshot: Record<string, Object> = {};
      for (const key of Object.keys(local)) snapshot[key] = local[key];
      snapshot['preferences'] = preferences;
      const saved: Record<string, Object> = JSON.parse(client.savePreferences(stateDirectory,
        local['revision'] as number, JSON.stringify(snapshot))) as Record<string, Object>;
      if (saved['ok'] !== true) {
        KeyboardLog.error(`shared preferences not saved: ${String(saved['error'])}`);
        return false;
      }
      return true;
    } catch (error) {
      KeyboardLog.error(`shared preferences not imported: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * 设置应用一侧：按当前的统计设置写出给键盘的开关文件。`reset` 为真表示刚清空了统计，清空次数加一，键盘看到后清空自己的计数。内容没变时不写，指纹也就不变。
   */
  static writeStatisticsSwitch(stateDirectory: string, reset: boolean): void {
    try {
      const reply: Record<string, Object> = JSON.parse(client.typingStatistics(JSON.stringify(
        SharedStateSync.statisticsRequest(stateDirectory, { operation: 'load' })))) as Record<string, Object>;
      if (reply['ok'] !== true) return;
      const value: Record<string, Object> = reply['value'] as Record<string, Object>;
      const path: string = `${stateDirectory}/${STATISTICS_SWITCH_FILE}`;
      const previousText: string | null = SharedStateSync.readText(path);
      const previous: StatisticsSwitch | null =
        previousText === null ? null : SharedStateMirrorPolicy.parseStatisticsSwitch(previousText);
      const retention: Object | undefined = value['retention'];
      const next: StatisticsSwitch = {
        enabled: value['enabled'] === true,
        retention: typeof retention === 'string' ? retention : 'forever',
        resets: (previous === null ? 0 : previous.resets) + (reset ? 1 : 0),
      };
      const text: string = JSON.stringify(next);
      if (text !== previousText) {
        SharedStateSync.writeText(path, text);
      }
    } catch (error) {
      KeyboardLog.error(`statistics switch not written: ${describeError(error)}`);
    }
  }

  /**
   * 键盘一侧：把设置应用的统计开关用到键盘自己的统计文件上。清空次数比上次导入的多，就先清空键盘的计数；然后开关和保留期与设置应用不同的照改。返回假时这一条下次再试。
   */
  static applyStatisticsSwitch(stateDirectory: string, text: string | null): boolean {
    if (text === null) return true;
    const next: StatisticsSwitch | null = SharedStateMirrorPolicy.parseStatisticsSwitch(text);
    if (next === null) return false;
    const previousText: string | null = SharedStateSync.readText(`${stateDirectory}/${STATISTICS_SWITCH_FILE}`);
    const previous: StatisticsSwitch | null =
      previousText === null ? null : SharedStateMirrorPolicy.parseStatisticsSwitch(previousText);
    try {
      // 第一次导入时不清空：键盘在共享沙箱启用之前记下的计数，不该因为设置应用早先清空过而被清掉。
      if (previous !== null && next.resets > previous.resets
        && !SharedStateSync.statisticsCall(stateDirectory, { operation: 'reset' })) {
        return false;
      }
      const reply: Record<string, Object> = JSON.parse(client.typingStatistics(JSON.stringify(
        SharedStateSync.statisticsRequest(stateDirectory, { operation: 'load' })))) as Record<string, Object>;
      if (reply['ok'] !== true) return false;
      const value: Record<string, Object> = reply['value'] as Record<string, Object>;
      if ((value['enabled'] === true) !== next.enabled
        && !SharedStateSync.statisticsCall(stateDirectory,
          { operation: 'set_enabled', enabled: next.enabled })) {
        return false;
      }
      if (value['retention'] !== next.retention
        && !SharedStateSync.statisticsCall(stateDirectory, {
          operation: 'set_retention', retention: next.retention, day: TypingStatisticsPolicy.day(new Date()),
        })) {
        return false;
      }
      return true;
    } catch (error) {
      KeyboardLog.error(`statistics switch not applied: ${describeError(error)}`);
      return false;
    }
  }

  /**
   * 设置应用一侧：把键盘的统计合并进自己的统计文件。持有统计存储的锁（与 client-services 的 `typing-statistics.lock` 是同一把 flock）再写，免得与工作线程上的 `summary`、`record_skin` 互相覆盖；锁被占着时下次再试。
   */
  private static adoptKeyboardStatistics(stateDirectory: string, keyboardText: string): boolean {
    let lock: fs.File | null = null;
    try {
      lock = fs.openSync(`${stateDirectory}/typing-statistics.lock`, fs.OpenMode.CREATE | fs.OpenMode.READ_WRITE);
      lock.tryLock(true);
      const path: string = `${stateDirectory}/typing-statistics.json`;
      const merged: string | null =
        SharedStateMirrorPolicy.mergeStatistics(keyboardText, SharedStateSync.readText(path));
      if (merged === null) return false;
      SharedStateSync.writeText(path, merged);
      return true;
    } catch (error) {
      KeyboardLog.warn(`keyboard statistics not adopted yet: ${describeError(error)}`);
      return false;
    } finally {
      if (lock !== null) {
        try {
          lock.unlock();
        } catch {
          // 没拿到锁时没有可解的锁。
        }
        NativeCleanup.close(lock);
      }
    }
  }

  /**
   * 设置应用一侧：把键盘发布的那本生词本并进背单词的生词本，再把这一版存成「上次并过的那一版」。键盘有一条还没用过的清走记录、说它已经清走了上一版时，先丢掉上一版：这时键盘那本里的词都是之后加的，包括用户在背单词里移出后又从键盘加回来的词。
   *
   * 先读清走记录再读生词本，与键盘发布时的复制顺序相反，读到新记录时生词本一定也是新的。生词本先复制到旁边，原生层并的就是这一份，成功后再换成「上一版」，存下的上一版与并过的内容一字不差。合并失败或设置应用那本已满时返回假，下次再试；不记成已并过，键盘也就不会清走这些词。同步调用：会等工作线程上正在写词书的请求（例如导入一份大词表）做完，生词本平常很小，这里不另起线程。
   */
  private static mergeKeyboardPersonalWordbook(mirror: SharedStateMirror, stateDirectory: string): boolean {
    const previousPath: string = `${stateDirectory}/${KEYBOARD_PERSONAL_WORDBOOK_PATH}`;
    const staging: string = `${previousPath}.${util.generateRandomUUID(false)}.tmp`;
    try {
      const parent: string = previousPath.substring(0, previousPath.lastIndexOf('/'));
      if (!fs.accessSync(parent)) {
        fs.mkdirSync(parent, true);
      }
      const markerText: string | null = mirror.publishedText('keyboard',
        SharedStateSync.keyboardEntry(KEYBOARD_PERSONAL_WORDBOOK_DRAINED_ENTRY));
      let previousText: string | null = SharedStateSync.readText(previousPath);
      // 每条清走记录只用一次，用过的那份按同一个相对路径存下：内容相同的生词本可能先后出现（清走以后又加回同样的词），旧记录再拿来比，会把之后存下的上一版也当成已清走。
      const seenPath: string = `${stateDirectory}/${KEYBOARD_PERSONAL_WORDBOOK_DRAINED_PATH}`;
      if (PersonalWordbookPolicy.unseenDrain(markerText, SharedStateSync.readText(seenPath)) && markerText !== null) {
        if (previousText !== null
          && PersonalWordbookPolicy.previousDrained(markerText, SharedStateMirrorPolicy.fingerprintText(previousText))) {
          fs.unlinkSync(previousPath);
          previousText = null;
        }
        SharedStateSync.writeText(seenPath, markerText);
      }
      const publishedPath: string = mirror.publishedFile('keyboard',
        SharedStateSync.keyboardEntry(KEYBOARD_PERSONAL_WORDBOOK_ENTRY));
      if (SharedStateSync.readText(publishedPath) === null) {
        // 键盘那本清空了：没有要并的。上一版留着，直到键盘的清走记录说它已经清走了。
        return true;
      }
      fs.copyFileSync(publishedPath, staging);
      if (!PersonalWordbookPolicy.mergeAccepted(client.personalWordbookMerge(PersonalWordbookPolicy.mergeRequest(
        stateDirectory, staging, previousText === null ? null : previousPath)))) {
        KeyboardLog.warn('keyboard personal wordbook not merged yet: refused or the wordbook is full');
        return false;
      }
      fs.renameSync(staging, previousPath);
      return true;
    } catch (error) {
      KeyboardLog.warn(`keyboard personal wordbook not merged yet: ${describeError(error)}`);
      return false;
    } finally {
      try {
        if (fs.accessSync(staging)) fs.unlinkSync(staging);
      } catch {
        // 只会留下这个唯一命名的暂存文件。
      }
    }
  }

  private static statisticsRequest(stateDirectory: string, action: StatisticsAction): StatisticsRequest {
    return { directory: stateDirectory, action: action };
  }

  private static statisticsCall(stateDirectory: string, action: StatisticsAction): boolean {
    const reply: Record<string, Object> = JSON.parse(client.typingStatistics(JSON.stringify(
      SharedStateSync.statisticsRequest(stateDirectory, action)))) as Record<string, Object>;
    if (reply['ok'] !== true) {
      KeyboardLog.error(`typing statistics ${action.operation} refused: ${String(reply['error'])}`);
      return false;
    }
    return true;
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
      KeyboardLog.error('keyboard.SharedStateSync.writeText failed');
      throw error as Error;
    }
  }
}

