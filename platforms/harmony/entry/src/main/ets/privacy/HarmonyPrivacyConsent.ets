/**
 * 本进程看到的隐私政策授权：读写 `files/state/privacy-consent.json`，并让等着联网的启动任务在同意之后才开始。规则在 `PrivacyConsentPolicy`。
 *
 * 设置应用和键盘各有一份 `files/state`，各自在本进程里持有一份。设置应用在弹窗里做决定、写文件、经共享沙箱发布；键盘在扩展创建时和每次获得焦点导入镜像之后重新读一次。拿不到共享目录时（签名不带 data-group-id、模拟器）键盘读不到授权，按没有授权处理：不联网，本机打字照常。
 *
 * 只用同步文件操作：文件只有几十个字节，调用方都在生命周期时刻调用。
 */
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import {
  ConsentRecord,
  ConsentState,
  PRIVACY_CONSENT_FILE,
  PRIVACY_POLICY_VERSION,
  PrivacyConsentPolicy
} from './PrivacyConsentPolicy';

export class HarmonyPrivacyConsent {
  private static record: ConsentRecord | null = null;
  private static waiters: (() => void)[] = [];

  /** 读本进程 `files/state` 里的授权记录；同意了当前版本时放行所有在等的任务。返回读到的记录。 */
  static load(stateDirectory: string): ConsentRecord | null {
    HarmonyPrivacyConsent.record = PrivacyConsentPolicy.parse(
      HarmonyPrivacyConsent.readText(HarmonyPrivacyConsent.path(stateDirectory)));
    HarmonyPrivacyConsent.release();
    return HarmonyPrivacyConsent.record;
  }

  /** 现在能不能发起受隐私政策约束的网络请求。 */
  static networkAllowed(): boolean {
    return PrivacyConsentPolicy.networkAllowed(HarmonyPrivacyConsent.record, PRIVACY_POLICY_VERSION);
  }

  /** 只作为本机上报授权的政策身份，不上传，也不供其他联网功能绕过政策门禁。 */
  static reportingIdentity(): string {
    return HarmonyPrivacyConsent.record === null ? 'undecided' : PrivacyConsentPolicy.serialize(HarmonyPrivacyConsent.record);
  }

  /** 打开设置应用时要不要弹窗。 */
  static shouldPrompt(): boolean {
    return PrivacyConsentPolicy.shouldPrompt(HarmonyPrivacyConsent.record, PRIVACY_POLICY_VERSION);
  }

  static state(): ConsentState {
    return PrivacyConsentPolicy.state(HarmonyPrivacyConsent.record, PRIVACY_POLICY_VERSION);
  }

  /**
   * 同意之后才继续的启动任务（匿名账号注册、使用统计的发送）在这里等。已经同意时立即放行；一直没有同意时一直不放行，这些任务就不发生。
   */
  static whenNetworkAllowed(): Promise<void> {
    if (HarmonyPrivacyConsent.networkAllowed()) {
      return Promise.resolve();
    }
    return new Promise<void>((resolve: () => void): void => {
      HarmonyPrivacyConsent.waiters.push(resolve);
    });
  }

  /** 设置应用里用户刚做的决定：写进记录文件，记在当前版本上。写不进去时返回假，本进程仍按这次的决定处理，下次启动会再问。 */
  static decide(stateDirectory: string, granted: boolean, nowMs: number): boolean {
    const record: ConsentRecord = PrivacyConsentPolicy.record(granted, PRIVACY_POLICY_VERSION, nowMs);
    HarmonyPrivacyConsent.record = record;
    const written: boolean = HarmonyPrivacyConsent.writeText(HarmonyPrivacyConsent.path(stateDirectory),
      PrivacyConsentPolicy.serialize(record));
    KeyboardLog.info(`privacy consent ${record.decision} for version ${record.version}`);
    HarmonyPrivacyConsent.release();
    return written;
  }

  private static release(): void {
    if (!HarmonyPrivacyConsent.networkAllowed() || HarmonyPrivacyConsent.waiters.length === 0) {
      return;
    }
    const waiting: (() => void)[] = HarmonyPrivacyConsent.waiters;
    HarmonyPrivacyConsent.waiters = [];
    for (const resolve of waiting) {
      resolve();
    }
  }

  private static path(stateDirectory: string): string {
    return `${stateDirectory}/${PRIVACY_CONSENT_FILE}`;
  }

  private static readText(path: string): string | null {
    try {
      const stat: fs.Stat = fs.lstatSync(path);
      if (stat.isSymbolicLink() || !stat.isFile() || stat.size > 4096) return null;
      return fs.readTextSync(path);
    } catch {
      // 没有文件就是从来没有做过决定。
      return null;
    }
  }

  private static writeText(path: string, text: string): boolean {
    const staging: string = `${path}.${util.generateRandomUUID(false)}.tmp`;
    try {
      const parent: string = path.substring(0, path.lastIndexOf('/'));
      if (parent.length > 0 && !fs.accessSync(parent)) {
        fs.mkdirSync(parent, true);
      }
      const handle: fs.File = fs.openSync(staging, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        fs.writeSync(handle.fd, text);
      } finally {
        fs.closeSync(handle);
      }
      fs.renameSync(staging, path);
      return true;
    } catch (error) {
      KeyboardLog.error(`privacy consent not saved: ${describeError(error)}`);
      return false;
    } finally {
      try {
        if (fs.accessSync(staging)) fs.unlinkSync(staging);
      } catch {
        // 只会留下这个唯一命名的暂存文件。
      }
    }
  }
}
