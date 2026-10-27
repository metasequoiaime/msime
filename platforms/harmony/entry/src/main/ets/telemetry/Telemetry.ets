import { NativeCleanup } from '../keyboard/NativeCleanup';
import bundleManager from '@ohos.bundle.bundleManager';
import hiAppEvent from '@ohos.hiviewdfx.hiAppEvent';
import process from '@ohos.process';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { CrashReport, TelemetryPolicy } from './TelemetryPolicy';
import { HarmonyPrivacyConsent } from '../privacy/HarmonyPrivacyConsent';

/**
 * 匿名使用情况报告（`usage_reporting` 偏好，默认开启）。
 *
 * 队列、随机安装 id、每日的 `active` 和会话记账都在 client-services 里，位于 `files/state/telemetry`，设置应用和键盘扩展共用这个目录；client-services 在两个进程之间为它加锁。一个会话就是一个键盘进程：键盘在扩展创建时开始会话，在扩展正常销毁时结束会话。设置应用只发送已经排队的内容。
 *
 * 两个进程都要先有隐私政策的授权（`HarmonyPrivacyConsent`）：没同意之前设置应用不发送，键盘不开始会话、不收崩溃报告；撤回同意之后什么都不再发送。
 *
 * 崩溃用的是 HarmonyOS 自己的报告。HiAppEvent 在下次启动时把上一次运行的 `APP_CRASH` 交给监听者，JavaScript 和原生崩溃都一样，所以这里不安装任何处理函数，也不改变崩溃结束进程的方式。键盘进程的崩溃如果在这个键盘开始自己的会话之前报告，就写成那个未结束会话的崩溃记录，成为 `session_crash`；其他的都是独立记录。记录在下次启动时作为 `crash` 事件发送。
 */

const PLATFORM: string = 'harmony';
const WATCHER_NAME: string = 'msime_crash_reports';
const CRASH_DIRECTORY: string = 'telemetry-crashes';
/** The keyboard's process id, remembered so the next start can tell its crash from the settings application's. */
const KEYBOARD_PROCESS_FILE: string = 'harmony-keyboard-process.json';
/**
 * How long the keyboard waits for the system's crash report before it closes the previous session.
 *
 * Closing it first would turn a crashed session into one that left only its marker, which counts as no session at all. A report that comes later is still sent, as a standalone crash.
 */
const BEGIN_DELAY_MS: number = 2000;

interface TelemetryRequest {
  directory: string;
  platform: string;
  version: string;
  preferences_directory: string;
  capture_only?: boolean;
  component?: string;
}

interface DirectoryRequest {
  directory: string;
}

interface CrashRecordRequest {
  directory: string;
  message: string;
  stack: string;
  pid?: number;
  outside_session?: boolean;
  crash_record_header?: string;
  diagnostic_id?: string;
}

interface BeginValue {
  enabled?: boolean;
}

interface BeginReply {
  ok: boolean;
  value?: BeginValue;
  error?: string;
}

interface FlagReply {
  ok: boolean;
  value?: boolean;
  error?: string;
}

interface RememberedProcess {
  pid: number;
}

function version(): string {
  try {
    return bundleManager.getBundleInfoForSelfSync(
      bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT).versionName;
  } catch (error) {
    KeyboardLog.error(`telemetry: version unreadable: ${describeError(error)}`);
    return '';
  }
}

export class HarmonyTelemetry {
  private static directory: string = '';
  private static preferences: string = '';
  private static began: boolean = false;
  private static enabled: boolean = false;
  private static previousKeyboardPid: number | null = null;
  private static beginTimer: number = -1;
  private static lastFlush: number = 0;
  private static watcher: hiAppEvent.Watcher | undefined = undefined;
  // 每次扩展创建和销毁都加一：销毁之后才等到授权的那次启动不再开始会话。
  private static startGeneration: number = 0;

  /** The telemetry directory under a process's shared state directory. */
  static directoryFor(stateDirectory: string): string {
    return `${stateDirectory}/telemetry`;
  }

  /** The settings application started: send what is queued, which also counts today's `active`. No session: that is the keyboard's. */
  static applicationStarted(stateDirectory: string): void {
    void HarmonyPrivacyConsent.whenNetworkAllowed().then((): void => {
      HarmonyTelemetry.directory = HarmonyTelemetry.directoryFor(stateDirectory);
      HarmonyTelemetry.preferences = stateDirectory;
      const request: TelemetryRequest = HarmonyTelemetry.request(stateDirectory);
      request.capture_only = true;
      request.component = 'application';
      const reply: string = client.telemetryBegin(JSON.stringify(request));
      try {
        const parsed: BeginReply = JSON.parse(reply) as BeginReply;
        HarmonyTelemetry.enabled = parsed.ok === true && parsed.value?.enabled === true;
      } catch { HarmonyTelemetry.enabled = false; }
      if (HarmonyTelemetry.enabled) HarmonyTelemetry.watchCrashes();
      HarmonyTelemetry.send(request);
    });
  }

  /** 键盘扩展创建了：先监听系统的崩溃报告，再开始本进程的会话。没有隐私政策的授权时等到键盘读到同意（之后某次获得焦点）再开始。 */
  static keyboardStarted(stateDirectory: string): void {
    const generation: number = ++HarmonyTelemetry.startGeneration;
    void HarmonyPrivacyConsent.whenNetworkAllowed().then((): void => {
      if (generation === HarmonyTelemetry.startGeneration) HarmonyTelemetry.startKeyboardSession(stateDirectory);
    });
  }

  private static startKeyboardSession(stateDirectory: string): void {
    HarmonyTelemetry.directory = HarmonyTelemetry.directoryFor(stateDirectory);
    HarmonyTelemetry.preferences = stateDirectory;
    HarmonyTelemetry.began = false;
    HarmonyTelemetry.enabled = false;
    HarmonyTelemetry.previousKeyboardPid = HarmonyTelemetry.readRememberedPid();
    // 独立准备只读取同意，不覆盖上一轮键盘会话；系统回调可以随时按 pid 认领。
    const request: TelemetryRequest = HarmonyTelemetry.request(stateDirectory);
    request.capture_only = true;
    try {
      const parsed: BeginReply = JSON.parse(client.telemetryBegin(JSON.stringify(request))) as BeginReply;
      HarmonyTelemetry.enabled = parsed.ok === true && parsed.value?.enabled === true;
    } catch { HarmonyTelemetry.enabled = false; }
    if (HarmonyTelemetry.enabled) HarmonyTelemetry.watchCrashes();
    HarmonyTelemetry.beginTimer = setTimeout((): void => {
      HarmonyTelemetry.beginTimer = -1;
      HarmonyTelemetry.begin();
    }, BEGIN_DELAY_MS);
  }

  /** A text field took the keyboard: a process that stays up for days still sends each day's `active`. */
  static keyboardShown(): void {
    if (!HarmonyTelemetry.began || !TelemetryPolicy.flushDue(HarmonyTelemetry.lastFlush, Date.now())) {
      return;
    }
    HarmonyTelemetry.send(HarmonyTelemetry.request(HarmonyTelemetry.preferences));
  }

  /** The keyboard extension is being destroyed normally: its session ended without a crash. */
  static keyboardStopped(): void {
    HarmonyTelemetry.startGeneration++;
    if (HarmonyTelemetry.beginTimer !== -1) {
      clearTimeout(HarmonyTelemetry.beginTimer);
      HarmonyTelemetry.beginTimer = -1;
    }
    if (HarmonyTelemetry.watcher !== undefined) {
      try {
        hiAppEvent.removeWatcher(HarmonyTelemetry.watcher);
      } catch (error) {
        KeyboardLog.error(`telemetry: crash watcher not removed: ${describeError(error)}`);
      }
      HarmonyTelemetry.watcher = undefined;
    }
    // Only a session this process began is this process's to end: ending before it began would close the crashed previous session as a normal one.
    if (!HarmonyTelemetry.began) {
      return;
    }
    HarmonyTelemetry.began = false;
    const request: DirectoryRequest = { directory: HarmonyTelemetry.directory };
    HarmonyTelemetry.logRefusal('end', client.telemetryEnd(JSON.stringify(request)));
  }

  /** The user just turned usage reporting off in settings: nothing queued is sent, now or later. */
  static reportingTurnedOff(stateDirectory: string): void {
    HarmonyTelemetry.enabled = false;
    const request: DirectoryRequest = { directory: HarmonyTelemetry.directoryFor(stateDirectory) };
    HarmonyTelemetry.logRefusal('clear', client.telemetryClear(JSON.stringify(request)));
  }

  private static request(stateDirectory: string): TelemetryRequest {
    return {
      directory: HarmonyTelemetry.directoryFor(stateDirectory),
      platform: PLATFORM,
      version: version(),
      preferences_directory: stateDirectory,
    };
  }

  private static begin(): void {
    const request: TelemetryRequest = HarmonyTelemetry.request(HarmonyTelemetry.preferences);
    const reply: string = client.telemetryBegin(JSON.stringify(request));
    let parsed: BeginReply;
    try {
      parsed = JSON.parse(reply) as BeginReply;
    } catch (error) {
      KeyboardLog.error(`telemetry: begin answer unreadable: ${describeError(error)}`);
      return;
    }
    if (parsed.ok !== true) {
      KeyboardLog.error(`telemetry: begin refused: ${parsed.error ?? 'unknown'}`);
      return;
    }
    HarmonyTelemetry.began = true;
    HarmonyTelemetry.enabled = parsed.value?.enabled === true;
    if (!HarmonyTelemetry.enabled) {
      return;
    }
    HarmonyTelemetry.rememberPid();
    HarmonyTelemetry.send(request);
  }

  private static send(request: TelemetryRequest): void {
    // 用户撤回了同意：排队的内容留在本机，不再发送。
    if (!HarmonyPrivacyConsent.networkAllowed()) {
      return;
    }
    HarmonyTelemetry.lastFlush = Date.now();
    client.telemetryFlush(JSON.stringify(request)).then((reply: string): void => {
      HarmonyTelemetry.logRefusal('flush', reply);
    }).catch((error: Error): void => {
      KeyboardLog.error(`telemetry: flush failed: ${describeError(error)}`);
    });
  }

  private static watchCrashes(): void {
    if (HarmonyTelemetry.watcher !== undefined) return;
    const watcher: hiAppEvent.Watcher = {
      name: WATCHER_NAME,
      appEventFilters: [{ domain: hiAppEvent.domain.OS, names: [hiAppEvent.event.APP_CRASH, hiAppEvent.event.APP_FREEZE] }],
      onReceive: (domain: string, groups: Array<hiAppEvent.AppEventGroup>): void => {
        for (const group of groups) {
          for (const info of group.appEventInfos) {
            const report: CrashReport | null = info.name === hiAppEvent.event.APP_FREEZE
              ? TelemetryPolicy.freezeReport(JSON.stringify(info.params))
              : TelemetryPolicy.crashReport(JSON.stringify(info.params));
            if (report !== null) {
              HarmonyTelemetry.recordCrash(report);
            }
          }
        }
      }
    };
    try {
      hiAppEvent.addWatcher(watcher);
      HarmonyTelemetry.watcher = watcher;
    } catch (error) {
      KeyboardLog.error(`telemetry: crash watcher refused: ${describeError(error)}`);
    }
  }

  private static recordCrash(report: CrashReport): void {
    if (!HarmonyTelemetry.enabled || !HarmonyPrivacyConsent.networkAllowed()) return;
    // 按系统报告的 pid 认领未结束的会话，不依赖两秒内是否已收到回调。
    const request: CrashRecordRequest = {
      directory: HarmonyTelemetry.directory, message: report.message, stack: report.stack,
      crash_record_header: TelemetryPolicy.recordText(report).split('\n')[0] + '\n',
      diagnostic_id: TelemetryPolicy.standaloneRecordName(report)
    };
    if (report.outsideSession === true) request.outside_session = true;
    else request.pid = report.pid;
    const reply: FlagReply | null = HarmonyTelemetry.flagReply('record', client.telemetryRecordCrash(JSON.stringify(request)));
    if (reply !== null) {
      HarmonyTelemetry.send(HarmonyTelemetry.request(HarmonyTelemetry.preferences));
      return;
    }
    HarmonyTelemetry.writeStandalone(report);
  }

  private static writeStandalone(report: CrashReport): void {
    const directory: string = `${HarmonyTelemetry.directory}/${CRASH_DIRECTORY}`;
    const path: string = `${directory}/${TelemetryPolicy.standaloneRecordName(report)}`;
    try {
      if (!HarmonyTelemetry.prepareTelemetryDirectory()) {
        return;
      }
      if (fs.accessSync(directory)) {
        const directoryStat: fs.Stat = fs.lstatSync(directory);
        if (directoryStat.isSymbolicLink() || !directoryStat.isDirectory()) {
          return;
        }
      } else {
        fs.mkdirSync(directory, true);
        const directoryStat: fs.Stat = fs.lstatSync(directory);
        if (directoryStat.isSymbolicLink() || !directoryStat.isDirectory()) {
          return;
        }
      }
      if (fs.accessSync(path)) {
        return;
      }
      HarmonyTelemetry.atomicWrite(path, TelemetryPolicy.recordText(report), false);
    } catch (error) {
      KeyboardLog.error(`telemetry: crash record not written: ${describeError(error)}`);
    }
  }

  private static readRememberedPid(): number | null {
    const path: string = `${HarmonyTelemetry.directory}/${KEYBOARD_PROCESS_FILE}`;
    let handle: fs.File | null = null;
    try {
      if (!HarmonyTelemetry.safeTelemetryDirectory()) {
        return null;
      }
      const expected: fs.Stat = fs.lstatSync(path);
      if (expected.isSymbolicLink() || !expected.isFile()) {
        return null;
      }
      handle = fs.openSync(path, fs.OpenMode.READ_ONLY);
      const stat: fs.Stat = fs.statSync(handle.fd);
      if (!stat.isFile() || stat.size > 256 || stat.ino !== expected.ino) return null;
      const buffer: ArrayBuffer = new ArrayBuffer(stat.size + 1);
      const count: number = fs.readSync(handle.fd, buffer);
      if (count !== stat.size) return null;
      const text: string = util.TextDecoder.create('utf-8').decodeToString(
        new Uint8Array(buffer, 0, count));
      return TelemetryPolicy.rememberedPid(text);
    } catch (error) {
      // An absent marker is the normal first-run state.
      try {
        if (fs.accessSync(path)) {
          KeyboardLog.error(`telemetry: keyboard process unreadable: ${describeError(error)}`);
        }
      } catch {
        // The directory or marker is unavailable.
      }
      return null;
    } finally {
      if (handle !== null) NativeCleanup.close(handle);
    }
  }

  private static rememberPid(): void {
    const remembered: RememberedProcess = { pid: process.pid };
    const path: string = `${HarmonyTelemetry.directory}/${KEYBOARD_PROCESS_FILE}`;
    try {
      if (!HarmonyTelemetry.prepareTelemetryDirectory()) {
        return;
      }
      HarmonyTelemetry.atomicWrite(path, JSON.stringify(remembered), true);
    } catch (error) {
      KeyboardLog.error(`telemetry: keyboard process not remembered: ${describeError(error)}`);
    }
  }

  private static atomicWrite(path: string, text: string, replaceExisting: boolean): boolean {
    try {
      const staging: string = `${path}.${util.generateRandomUUID(false)}.tmp`;
      let renamed: boolean = false;
      try {
        const file: fs.File = fs.openSync(staging,
          fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
        try {
          fs.writeSync(file.fd, text);
        } finally {
          fs.closeSync(file);
        }
        if (!replaceExisting && fs.accessSync(path)) return false;
        fs.renameSync(staging, path);
        renamed = true;
        return true;
      } finally {
        if (!renamed) {
          try {
            if (fs.accessSync(staging)) fs.unlinkSync(staging);
          } catch {
            // 遥测写入失败时，最多留下以唯一名称创建的暂存文件。
          }
        }
      }
    } catch (error) {
      KeyboardLog.error('telemetry.Telemetry.atomicWrite failed');
      throw error as Error;
    }
  }

  /** Ensure the shared telemetry directory itself is a real directory before following it. */
  private static prepareTelemetryDirectory(): boolean {
    try {
      if (!fs.accessSync(HarmonyTelemetry.directory)) {
        fs.mkdirSync(HarmonyTelemetry.directory, true);
      }
      return HarmonyTelemetry.safeTelemetryDirectory();
    } catch {
      return false;
    }
  }

  private static safeTelemetryDirectory(): boolean {
    try {
      const stat: fs.Stat = fs.lstatSync(HarmonyTelemetry.directory);
      return !stat.isSymbolicLink() && stat.isDirectory();
    } catch {
      return false;
    }
  }

  /** The answer to a call whose value is a flag, or null (logged) when it was refused or unreadable. */
  private static flagReply(step: string, reply: string): FlagReply | null {
    try {
      const parsed: FlagReply = JSON.parse(reply) as FlagReply;
      if (parsed.ok === true) {
        return parsed;
      }
      KeyboardLog.error(`telemetry: ${step} refused: ${parsed.error ?? 'unknown'}`);
    } catch (error) {
      KeyboardLog.error(`telemetry: ${step} answer unreadable: ${describeError(error)}`);
    }
    return null;
  }

  private static logRefusal(step: string, reply: string): void {
    HarmonyTelemetry.flagReply(step, reply);
  }
}
