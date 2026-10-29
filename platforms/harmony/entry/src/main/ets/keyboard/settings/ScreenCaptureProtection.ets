import fs from '@ohos.file.fs';
import util from '@ohos.util';
import common from '@ohos.app.ability.common';
import window from '@ohos.window';
import inputMethodEngine from '@ohos.inputMethodEngine';
import { KeyboardLog, describeError } from '../KeyboardLog';
import {
  SCREEN_CAPTURE_PROTECTION_FILE,
  SCREEN_CAPTURE_PROTECTION_MAX_BYTES,
  ScreenCaptureProtectionPolicy,
  ScreenCaptureProtectionState
} from './ScreenCaptureProtectionPolicy';

/**
 * 「防止截屏」在本进程里的读写和效果：设置应用写 `files/state/screen-capture-protection.json`、给自己的窗口设隐私模式；键盘读镜像过来的那一份、给三块面板设隐私模式。切换规则在 `ScreenCaptureProtectionPolicy`。
 *
 * 隐私模式要 `ohos.permission.PRIVACY_WINDOW`（normal 级、system_grant，在 `module.json5` 里声明即授予）。设置失败（权限不在、窗口已销毁）只记日志：这是系统边界，不能因此让设置页或键盘起不来。
 *
 * 只用同步文件操作：文件只有几十个字节，调用方都在生命周期时刻或一次用户操作里调用。
 */
export class ScreenCaptureProtection {
  /** 每块面板上次设成功的隐私模式；键是面板对象，面板销毁后随之回收。 */
  private static readonly panelModes: WeakMap<inputMethodEngine.Panel, boolean> =
    new WeakMap<inputMethodEngine.Panel, boolean>();

  static load(stateDirectory: string): ScreenCaptureProtectionState {
    return ScreenCaptureProtectionPolicy.parse(
      ScreenCaptureProtection.readText(`${stateDirectory}/${SCREEN_CAPTURE_PROTECTION_FILE}`));
  }

  /** 写入新状态；写不进去时返回假，调用方保持原值。 */
  static save(stateDirectory: string, state: ScreenCaptureProtectionState): boolean {
    return ScreenCaptureProtection.writeText(`${stateDirectory}/${SCREEN_CAPTURE_PROTECTION_FILE}`,
      ScreenCaptureProtectionPolicy.serialize(state));
  }

  /**
   * 给输入法面板（软键盘、候选窗、浮动工具条、输入模式提示）设或取消隐私模式。每次编辑框获得焦点都会调用，而 `setPrivacyMode` 是一次同步的窗口管理 IPC、正好在 `panel.show()` 之前；所以记下每块面板上次设成功的值，没变就不再设。面板重建后是新对象，不在表里，照常设一次；设失败不记，下一次再试。
   */
  static protectPanel(panel: inputMethodEngine.Panel, enabled: boolean, name: string): void {
    if (ScreenCaptureProtection.panelModes.get(panel) === enabled) return;
    try {
      panel.setPrivacyMode(enabled);
      ScreenCaptureProtection.panelModes.set(panel, enabled);
    } catch (error) {
      KeyboardLog.error(`${name} privacy mode not applied: ${describeError(error)}`);
    }
  }

  /** 给设置应用的窗口设或取消隐私模式。 */
  static protectWindow(main: window.Window, enabled: boolean): void {
    try {
      main.setWindowPrivacyMode(enabled).catch((error: Object): void => {
        KeyboardLog.error(`settings window privacy mode not applied: ${describeError(error)}`);
      });
    } catch (error) {
      KeyboardLog.error(`settings window privacy mode not applied: ${describeError(error)}`);
    }
  }

  /** 设置页写入成功后当场更新设置应用的窗口，不等下一次启动。 */
  static protectLastWindow(context: common.UIAbilityContext, enabled: boolean): void {
    window.getLastWindow(context).then((main: window.Window): void => {
      ScreenCaptureProtection.protectWindow(main, enabled);
    }).catch((error: Object): void => {
      KeyboardLog.error(`settings window unavailable for privacy mode: ${describeError(error)}`);
    });
  }

  private static readText(path: string): string | null {
    try {
      const stat: fs.Stat = fs.lstatSync(path);
      if (stat.isSymbolicLink() || !stat.isFile() || stat.size > SCREEN_CAPTURE_PROTECTION_MAX_BYTES) return null;
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
      KeyboardLog.error(`screen capture protection not saved: ${describeError(error)}`);
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

