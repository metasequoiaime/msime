import fs from '@ohos.file.fs';
import util from '@ohos.util';
import { KeyboardLog, describeError } from '../KeyboardLog';
import {
  CLIPBOARD_DELETE_AUTHENTICATION_FILE,
  CLIPBOARD_DELETE_AUTHENTICATION_MAX_BYTES,
  ClipboardDeleteAuthenticationPolicy
} from './ClipboardDeleteAuthenticationPolicy';

/**
 * 「剪贴板删除数据验证」在本进程里的读写：设置应用写 `files/state/clipboard-delete-authentication.json`，键盘读镜像过来的那一份。切换和删除规则在 `ClipboardDeleteAuthenticationPolicy`。
 *
 * 只用同步文件操作：文件只有十几个字节，调用方都在一次用户操作里调用。
 */
export class ClipboardDeleteAuthentication {
  static load(stateDirectory: string): boolean {
    return ClipboardDeleteAuthenticationPolicy.parse(
      ClipboardDeleteAuthentication.readText(`${stateDirectory}/${CLIPBOARD_DELETE_AUTHENTICATION_FILE}`));
  }

  /** 写入新值；写不进去时返回假，调用方保持原值。 */
  static save(stateDirectory: string, enabled: boolean): boolean {
    return ClipboardDeleteAuthentication.writeText(`${stateDirectory}/${CLIPBOARD_DELETE_AUTHENTICATION_FILE}`,
      ClipboardDeleteAuthenticationPolicy.serialize(enabled));
  }

  private static readText(path: string): string | null {
    try {
      const stat: fs.Stat = fs.lstatSync(path);
      if (stat.isSymbolicLink() || !stat.isFile() || stat.size > CLIPBOARD_DELETE_AUTHENTICATION_MAX_BYTES) {
        return null;
      }
      return fs.readTextSync(path);
    } catch {
      // 没有文件就是从来没有打开过。
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
      KeyboardLog.error(`clipboard delete authentication not saved: ${describeError(error)}`);
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
